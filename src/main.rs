use std::fs;
use std::process::ExitCode;
use std::collections::{HashMap, HashSet, VecDeque};
use derive_more::{Index, Deref, Constructor};
use clap::{Parser, Subcommand};
use dot_structures::{Id, Graph, Stmt, Edge, EdgeTy, Vertex, NodeId};

#[derive(Debug, Clone, Index, Deref, Constructor)]
struct Item<T: Clone> {
    #[deref]
    data: T,
    #[index]
    deps: Vec<usize>,
}

fn id_to_string(id: Id) -> String {
    match id {
        Id::Html(s) => s,
        Id::Escaped(s) => s,
        Id::Plain(s) => s,
        Id::Anonymous(s) => s,
    }
}

fn from_graphviz(graph: &Graph) -> Vec<Item<String>> {
    let mut items = Vec::new();
    let mut indices = HashMap::new();
    let stmts = match graph {
        Graph::DiGraph { stmts, .. } => stmts,
        Graph::Graph { .. } => panic!("Only directed graphs are supported"),
    };
    for stmt in stmts {
        let edge_ty = match stmt {
            Stmt::Edge(Edge { ty, .. }) => ty,
            _ => panic!("Statement is not supported: {:?}", stmt),
        };
        match edge_ty {
            EdgeTy::Pair(Vertex::N(NodeId(a, _)), Vertex::N(NodeId(b, _))) => {
                let a = id_to_string(a.clone());
                let b = id_to_string(b.clone());

                let index_a = *indices.entry(a.clone()).or_insert_with(|| items.len());
                if index_a == items.len() {
                    items.push(Item::new(a, Vec::new()));
                }

                let index_b = *indices.entry(b.clone()).or_insert_with(|| items.len());
                if index_b == items.len() {
                    items.push(Item::new(b, Vec::new()));
                }

                items[index_a].deps.push(index_b);
            },
            EdgeTy::Chain(_) => todo!(),
            // If we don't match a chain or a pair of nodes, we are
            // an edge connected to a subgraph.
            _ => panic!("Subgraphs are not supported"),
        }
    }

    items
}

fn detect_cycle<T: Clone>(items: &[Item<T>]) -> Option<(usize, usize)> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();

    for start in 0..items.len() {
        if visited.contains(&start) {
            continue;
        }

        stack.push((start, 0, None));
        let mut path = HashSet::new();

        while let Some((node, dep_index, parent)) = stack.pop() {
            if dep_index == 0 {
                if path.contains(&node) {
                    return parent.map(|p| (node, p));
                }
                path.insert(node);
                visited.insert(node);
            }

            if dep_index < items[node].deps.len() {
                stack.push((node, dep_index + 1, parent));
                let next_node = items[node].deps[dep_index];
                stack.push((next_node, 0, Some(node)));
            } else {
                path.remove(&node);
            }
        }
    }

    None
}

fn sort_items<T: Clone>(items: &[Item<T>]) -> Vec<Item<T>> {
    let mut graph = HashMap::new();
    let mut in_degree = vec![0; items.len()];

    for (index, item) in items.iter().enumerate() {
        for &dep_index in &item.deps {
            graph.entry(dep_index).or_insert_with(Vec::new).push(index);
            in_degree[index] += 1;
        }
    }

    let mut queue = VecDeque::new();
    for (index, &degree) in in_degree.iter().enumerate() {
        if degree == 0 {
            queue.push_back(index);
        }
    }

    let mut sorted_indices = Vec::new();
    while let Some(index) = queue.pop_front() {
        if let Some(deps) = graph.get(&index) {
            for &dep_index in deps {
                in_degree[dep_index] -= 1;
                if in_degree[dep_index] == 0 {
                    queue.push_back(dep_index);
                }
            }
        }
        sorted_indices.push(index);
    }

    sorted_indices.into_iter().map(|index| items[index].clone()).collect()
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Check {
        input_path: String,
    },
    Sort {
        input_path: String,
    },
}

fn main() -> ExitCode {
    let args = Args::parse();

    match args.command {
        Command::Check { input_path } => {
            let contents = fs::read_to_string(input_path).unwrap();
            let graph = graphviz_rust::parse(&contents).unwrap();

            let items = from_graphviz(&graph);

            if let Some((a, b)) = detect_cycle(&items) {
                eprintln!("Circular dependency detected between {} and {}", items[a].data, items[b].data);
            } else {
                println!("The graph has no circular dependencies");
            }

            ExitCode::SUCCESS
        },
        Command::Sort { input_path } => {
            let contents = fs::read_to_string(input_path).unwrap();
            let graph = graphviz_rust::parse(&contents).unwrap();

            let items = from_graphviz(&graph);

            if let Some((a, b)) = detect_cycle(&items) {
                eprintln!("ERROR: Circular dependency detected between {} and {}", *items[a], *items[b]);
                eprintln!("           Cannot sort a graph with cycles");
                return ExitCode::FAILURE;
            }

            let sorted = sort_items(&items);
            for item in sorted {
                println!("{}", *item);
            }

            ExitCode::SUCCESS
        },
    }
}

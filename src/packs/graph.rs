use super::Configuration;
use serde::Serialize;

/// A single pack (node) in the dependency graph.
#[derive(Serialize, Debug, PartialEq, Eq)]
struct GraphNode {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    layer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
}

/// A directed edge `from -> to`. `kind` records how the dependency is expressed
/// in the source pack's configuration:
/// - `declared`: listed under `dependencies:` in package.yml
/// - `ignored`:  listed under `ignored_dependencies:` in package.yml
/// - `todo`:     a recorded violation in the source pack's package_todo.yml
///
/// This is raw, uninterpreted output: no cycle detection, SCC decomposition, or
/// simulation is performed here — downstream tools compute those from the graph.
#[derive(Serialize, Debug, PartialEq, Eq)]
struct GraphEdge {
    from: String,
    to: String,
    kind: String,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
struct Graph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

/// Build the whole-repo pack dependency graph from the already-parsed pack set.
///
/// Output is fully ordered — nodes by `name`, edges by `(from, to, kind)` — so
/// that two runs against the same code produce byte-identical output (stable hash),
/// independent of the parsed collections' iteration order.
fn build(configuration: &Configuration) -> Graph {
    let mut nodes: Vec<GraphNode> = configuration
        .pack_set
        .packs
        .iter()
        .map(|pack| GraphNode {
            name: pack.name.clone(),
            layer: pack.layer.clone(),
            owner: pack.owner.clone(),
        })
        .collect();
    nodes.sort_by(|a, b| a.name.cmp(&b.name));

    let mut edges: Vec<GraphEdge> = Vec::new();
    for pack in &configuration.pack_set.packs {
        for to in &pack.dependencies {
            edges.push(GraphEdge {
                from: pack.name.clone(),
                to: to.clone(),
                kind: "declared".to_owned(),
            });
        }
        for to in &pack.ignored_dependencies {
            edges.push(GraphEdge {
                from: pack.name.clone(),
                to: to.clone(),
                kind: "ignored".to_owned(),
            });
        }
        for to in pack.package_todo.violations_by_defining_pack.keys() {
            edges.push(GraphEdge {
                from: pack.name.clone(),
                to: to.clone(),
                kind: "todo".to_owned(),
            });
        }
    }
    edges.sort_by(|a, b| {
        a.from
            .cmp(&b.from)
            .then_with(|| a.to.cmp(&b.to))
            .then_with(|| a.kind.cmp(&b.kind))
    });

    Graph { nodes, edges }
}

fn to_json(graph: &Graph) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(graph)?)
}

/// Print the pack dependency graph as deterministic JSON to stdout.
pub(crate) fn dump(configuration: &Configuration) -> anyhow::Result<()> {
    println!("{}", to_json(&build(configuration))?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packs::configuration;
    use std::path::PathBuf;

    fn config_for(fixture: &str) -> Configuration {
        configuration::get(
            PathBuf::from(fixture)
                .canonicalize()
                .expect("Could not canonicalize path")
                .as_path(),
        )
        .unwrap()
    }

    #[test]
    fn graph_output_is_deterministic() {
        let configuration = config_for("tests/fixtures/simple_app");
        let first = to_json(&build(&configuration)).unwrap();
        let second = to_json(&build(&configuration)).unwrap();
        assert_eq!(
            first, second,
            "graph JSON must be byte-identical across runs"
        );
    }

    #[test]
    fn nodes_and_edges_are_ordered() {
        let configuration = config_for("tests/fixtures/simple_app");
        let graph = build(&configuration);

        let node_names: Vec<&String> =
            graph.nodes.iter().map(|n| &n.name).collect();
        let mut sorted_names = node_names.clone();
        sorted_names.sort();
        assert_eq!(node_names, sorted_names, "nodes must be ordered by name");

        let edge_keys: Vec<(&String, &String, &String)> = graph
            .edges
            .iter()
            .map(|e| (&e.from, &e.to, &e.kind))
            .collect();
        let mut sorted_keys = edge_keys.clone();
        sorted_keys.sort();
        assert_eq!(
            edge_keys, sorted_keys,
            "edges must be ordered by (from, to, kind)"
        );
    }

    #[test]
    fn includes_declared_edges_and_nodes() {
        let configuration = config_for("tests/fixtures/simple_app");
        let graph = build(&configuration);

        assert!(
            graph.nodes.iter().any(|n| n.name == "packs/foo"),
            "expected a node for packs/foo"
        );
        // In simple_app, packs/foo declares a dependency on packs/baz
        // (mirrors dependencies.rs::find_explicit_dependencies).
        assert!(
            graph.edges.iter().any(|e| e.from == "packs/foo"
                && e.to == "packs/baz"
                && e.kind == "declared"),
            "expected declared edge packs/foo -> packs/baz"
        );
    }

    #[test]
    fn includes_todo_edges() {
        let configuration = config_for("tests/fixtures/contains_package_todo");
        let graph = build(&configuration);

        // packs/foo records a violation whose defining pack is packs/bar
        // (mirrors dependencies.rs::find_implicit_dependencies).
        assert!(
            graph.edges.iter().any(|e| e.from == "packs/foo"
                && e.to == "packs/bar"
                && e.kind == "todo"),
            "expected todo edge packs/foo -> packs/bar"
        );
    }
}

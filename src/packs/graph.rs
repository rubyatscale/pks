//! JSON output for `pks graph`.
//!
//! Serializes the whole-repo pack dependency graph (nodes + declared/ignored/todo
//! edges) to JSON. Output is fully ordered — nodes by `name`, edges by
//! `(from, to, kind)` — so repeated runs on unchanged config produce byte-identical
//! output (stable hash). This is raw, uninterpreted output: no cycle detection, SCC
//! decomposition, or simulation is performed here — downstream tools compute those
//! from the graph.
//!
//! See `schema/graph-output.json` for the JSON Schema specification.

use super::Configuration;
use serde::Serialize;

/// How a dependency edge is expressed in the source pack's configuration.
#[derive(Serialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum EdgeKind {
    /// Listed under `dependencies:` in package.yml.
    Declared,
    /// Listed under `ignored_dependencies:` in package.yml.
    Ignored,
    /// A recorded violation in the source pack's package_todo.yml.
    Todo,
}

/// A single pack (node) in the dependency graph.
#[derive(Serialize, Debug, PartialEq, Eq)]
struct GraphNode {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    layer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner: Option<String>,
}

/// A directed edge `from -> to`, tagged by how the dependency is expressed.
#[derive(Serialize, Debug, PartialEq, Eq)]
struct GraphEdge {
    from: String,
    to: String,
    kind: EdgeKind,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
struct Graph {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

/// Build the whole-repo pack dependency graph from the already-parsed pack set,
/// fully ordered for deterministic output.
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
                kind: EdgeKind::Declared,
            });
        }
        for to in &pack.ignored_dependencies {
            edges.push(GraphEdge {
                from: pack.name.clone(),
                to: to.clone(),
                kind: EdgeKind::Ignored,
            });
        }
        for to in pack.package_todo.violations_by_defining_pack.keys() {
            edges.push(GraphEdge {
                from: pack.name.clone(),
                to: to.clone(),
                kind: EdgeKind::Todo,
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

/// Write the pack dependency graph as compact JSON to `writer`.
fn write_graph<W: std::io::Write>(
    configuration: &Configuration,
    writer: W,
) -> anyhow::Result<()> {
    // Compact, raw structured data (matches `pks check -o json`); consumers format as needed.
    serde_json::to_writer(writer, &build(configuration))?;
    Ok(())
}

/// Print the pack dependency graph as deterministic JSON to stdout.
pub(crate) fn dump(configuration: &Configuration) -> anyhow::Result<()> {
    write_graph(configuration, std::io::stdout())
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

    fn json_bytes(configuration: &Configuration) -> Vec<u8> {
        let mut buf = Vec::new();
        write_graph(configuration, &mut buf).unwrap();
        buf
    }

    #[test]
    fn graph_output_is_deterministic() {
        let configuration = config_for("tests/fixtures/simple_app");
        assert_eq!(
            json_bytes(&configuration),
            json_bytes(&configuration),
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

        let edge_keys: Vec<(&String, &String, EdgeKind)> = graph
            .edges
            .iter()
            .map(|e| (&e.from, &e.to, e.kind))
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
        // In simple_app, packs/foo declares a dependency on packs/baz.
        assert!(
            graph.edges.iter().any(|e| e.from == "packs/foo"
                && e.to == "packs/baz"
                && e.kind == EdgeKind::Declared),
            "expected declared edge packs/foo -> packs/baz"
        );
    }

    #[test]
    fn includes_todo_edges() {
        let configuration = config_for("tests/fixtures/contains_package_todo");
        let graph = build(&configuration);

        // packs/foo records a violation whose defining pack is packs/bar.
        assert!(
            graph.edges.iter().any(|e| e.from == "packs/foo"
                && e.to == "packs/bar"
                && e.kind == EdgeKind::Todo),
            "expected todo edge packs/foo -> packs/bar"
        );
    }
}

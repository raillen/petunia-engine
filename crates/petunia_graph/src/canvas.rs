//! The graph as boxes and wires: the data the node canvas draws.
//!
//! Positions come from the layout file when it has one; missing ones are computed from the wires,
//! deterministically (events on the left, what they run to the right), so a graph without a layout
//! still opens as a readable picture and a test can compare the result byte for byte.

use std::collections::{BTreeMap, VecDeque};

use serde_json::{Value as Json, json};

use crate::catalog::{Catalog, Direction, PinKind};
use crate::compile::{Severity, validate};
use crate::edit::is_event;
use crate::layout::GraphLayout;
use crate::model::GraphDocument;
use crate::outline::{class_name, type_name};

/// Distance between columns and between rows of the computed layout, in canvas units.
const COLUMN: f64 = 300.0;
const ROW: f64 = 180.0;

/// The whole picture: every node with its place and pins, and every wire.
pub fn canvas(document: &GraphDocument, catalog: &Catalog, layout: Option<&GraphLayout>) -> Json {
    let computed = computed_positions(document, catalog);
    let diagnostics = validate(document, catalog);
    let mut nodes = Vec::with_capacity(document.nodes.len());
    for node in &document.nodes {
        let spec = catalog.get(&node.kind);
        let (x, y) = layout
            .and_then(|layout| layout.position(node.id))
            .or_else(|| computed.get(&node.id).copied())
            .unwrap_or((0.0, 0.0));
        let problems: Vec<Json> = diagnostics
            .iter()
            .filter(|finding| finding.node == Some(node.id))
            .map(|finding| {
                json!({
                    "severity": if finding.severity == Severity::Error { "error" } else { "warning" },
                    "pin": finding.pin,
                    "code": finding.code,
                    "message": finding.message,
                })
            })
            .collect();
        let pins: Vec<Json> = spec.map_or_else(Vec::new, |spec| {
            spec.pins
                .iter()
                .map(|pin| {
                    json!({
                        "name": pin.name,
                        "label": pin.label,
                        "direction": if pin.direction == Direction::In { "in" } else { "out" },
                        "exec": pin.kind == PinKind::Exec,
                        "type": type_name(pin.kind),
                    })
                })
                .collect()
        });
        nodes.push(json!({
            "node": node.id,
            "kind": node.kind,
            "title": spec.map_or_else(|| node.kind.clone(), |spec| spec.title.clone()),
            "class": spec.map_or("ghost", |spec| class_name(spec.class)),
            "summary": spec.map_or("", |spec| spec.summary.as_str()),
            "x": x,
            "y": y,
            "pins": pins,
            "problems": problems,
        }));
    }
    let mut edges: Vec<Json> = document
        .edges
        .iter()
        .map(|edge| {
            let kind = document
                .node(edge.from.node)
                .and_then(|node| catalog.get(&node.kind))
                .and_then(|spec| spec.pin(&edge.from.pin, Direction::Out))
                .map(|pin| pin.kind);
            json!({
                "from": { "node": edge.from.node, "pin": edge.from.pin },
                "to": { "node": edge.to.node, "pin": edge.to.pin },
                "kind": if kind == Some(PinKind::Exec) { "exec" } else { "data" },
                "type": kind.map_or("", type_name),
            })
        })
        .collect();
    edges.sort_by_key(|edge| edge.to_string());
    json!({ "nodes": nodes, "edges": edges })
}

/// Where each node goes when the layout file does not say: shortest distance from an event, then
/// order of discovery. Nodes no event reaches line up after the reachable ones.
fn computed_positions(document: &GraphDocument, catalog: &Catalog) -> BTreeMap<u32, (f64, f64)> {
    let mut outgoing: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for edge in &document.edges {
        outgoing
            .entry(edge.from.node)
            .or_default()
            .push(edge.to.node);
    }
    let mut events: Vec<u32> = document
        .nodes
        .iter()
        .filter(|node| catalog.get(&node.kind).is_some_and(is_event))
        .map(|node| node.id)
        .collect();
    events.sort_unstable();
    let mut depth: BTreeMap<u32, usize> = BTreeMap::new();
    let mut queue: VecDeque<u32> = VecDeque::new();
    for event in events {
        depth.insert(event, 0);
        queue.push_back(event);
    }
    while let Some(id) = queue.pop_front() {
        let level = depth.get(&id).copied().unwrap_or(0) + 1;
        let mut next = outgoing.get(&id).cloned().unwrap_or_default();
        next.sort_unstable();
        next.dedup();
        for target in next {
            if depth.contains_key(&target) || document.node(target).is_none() {
                continue;
            }
            depth.insert(target, level);
            queue.push_back(target);
        }
    }
    let mut rows: BTreeMap<usize, usize> = BTreeMap::new();
    let mut placed = BTreeMap::new();
    let mut ids: Vec<(usize, u32)> = depth.iter().map(|(id, level)| (*level, *id)).collect();
    ids.sort_unstable();
    for (level, id) in ids {
        let row = rows.entry(level).or_insert(0);
        placed.insert(id, (level as f64 * COLUMN, *row as f64 * ROW));
        *row += 1;
    }
    // Everything no event reaches (a loose chain, a value nothing reads): after the rest.
    let last = depth.values().copied().max().unwrap_or(0) + 1;
    let mut loose: Vec<u32> = document
        .nodes
        .iter()
        .map(|node| node.id)
        .filter(|id| !placed.contains_key(id))
        .collect();
    loose.sort_unstable();
    for (row, id) in loose.into_iter().enumerate() {
        placed.insert(id, (last as f64 * COLUMN, row as f64 * ROW));
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{ApiSpec, EventSpec};
    use crate::model::{GraphEdge, GraphNode, PinRef};

    fn catalog() -> Catalog {
        Catalog::new(
            &[ApiSpec {
                module: "transform".to_string(),
                name: "rotate_y".to_string(),
                params: vec![
                    ("entity".to_string(), "Int".to_string()),
                    ("degrees".to_string(), "Float".to_string()),
                ],
                returns: "None".to_string(),
                summary: "Gira".to_string(),
                permission: "world.write".to_string(),
            }],
            &[EventSpec {
                handler: "on_update".to_string(),
                title: "A cada quadro".to_string(),
                summary: "Todo quadro".to_string(),
                params: vec![
                    ("entity".to_string(), "Int".to_string()),
                    ("dt".to_string(), "Float".to_string()),
                ],
            }],
        )
    }

    fn graph() -> GraphDocument {
        let mut document = GraphDocument::new();
        document.nodes.push(GraphNode {
            id: 1,
            kind: "event.on_update".to_string(),
            params: BTreeMap::new(),
        });
        document.nodes.push(GraphNode {
            id: 2,
            kind: "api.transform.rotate_y".to_string(),
            params: BTreeMap::new(),
        });
        document.edges.push(GraphEdge {
            from: PinRef {
                node: 1,
                pin: "then".to_string(),
            },
            to: PinRef {
                node: 2,
                pin: "in".to_string(),
            },
        });
        document
    }

    #[test]
    fn the_canvas_lists_every_node_and_wire_and_computes_positions() {
        let picture = canvas(&graph(), &catalog(), None);
        let nodes = picture["nodes"].as_array().expect("nodes");
        assert_eq!(nodes.len(), 2);
        let event = &nodes[0];
        assert_eq!(event["class"], "event");
        assert_eq!(event["pins"].as_array().expect("pins").len(), 3);
        let step = &nodes[1];
        assert_eq!(step["class"], "action");
        assert!(
            step["x"].as_f64().expect("x") > event["x"].as_f64().expect("x"),
            "the action sits to the right of the event"
        );
        let edges = picture["edges"].as_array().expect("edges");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0]["kind"], "exec");
        // Same graph, same bytes: the picture is deterministic.
        assert_eq!(picture, canvas(&graph(), &catalog(), None));
    }

    #[test]
    fn a_saved_layout_wins_over_the_computed_one() {
        let mut layout = GraphLayout::new();
        layout.place(2, 999.0, -40.0).expect("place");
        let picture = canvas(&graph(), &catalog(), Some(&layout));
        let step = picture["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .find(|node| node["node"] == 2)
            .cloned()
            .expect("node 2");
        assert_eq!(step["x"], 999.0);
        assert_eq!(step["y"], -40.0);
    }
}

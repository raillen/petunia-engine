//! Where the nodes sit on the canvas: `scripts/<name>.layout.json`, beside the graph.
//!
//! Positions are kept out of the graph file so moving a box never hides a change of logic in a
//! diff, and losing this file only costs the arrangement, never the logic. Hostile input is
//! limited like the graph: size, node count and finite coordinates.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{GraphDocument, GraphError, MAX_NODES};

pub const LAYOUT_SCHEMA_VERSION: u32 = 1;
pub const MAX_LAYOUT_BYTES: usize = 1 << 19;
/// A node farther than this from the origin is a broken file, not a canvas.
pub const MAX_COORDINATE: f64 = 1.0e6;

fn refuse<T>(message: impl Into<String>) -> Result<T, GraphError> {
    Err(GraphError(message.into()))
}

/// One node's place on the canvas, in canvas units (pixels at 100 % zoom).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutNode {
    pub x: f64,
    pub y: f64,
}

/// A visual group: a label and the nodes it holds. The frame is computed from the member boxes,
/// so moving a member never leaves the frame behind; the group itself has no coordinates.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutGroup {
    pub id: u32,
    pub label: String,
    #[serde(default)]
    pub nodes: Vec<u32>,
}

/// The saved layout: a position per node id. Field order is the order of the saved file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphLayout {
    pub schema_version: u32,
    #[serde(default)]
    pub nodes: BTreeMap<u32, LayoutNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<LayoutGroup>,
}

impl Default for GraphLayout {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphLayout {
    pub fn new() -> Self {
        Self {
            schema_version: LAYOUT_SCHEMA_VERSION,
            nodes: BTreeMap::new(),
            groups: Vec::new(),
        }
    }

    /// Reads a saved layout. A broken file is refused, never silently half-applied.
    pub fn parse(text: &str) -> Result<Self, GraphError> {
        if text.len() > MAX_LAYOUT_BYTES {
            return refuse(format!(
                "o layout passa de {} KB; apague-o para o editor recalcular as posições",
                MAX_LAYOUT_BYTES / 1024
            ));
        }
        let layout: GraphLayout = serde_json::from_str(text)
            .map_err(|error| GraphError(format!("o arquivo de layout não é válido: {error}")))?;
        layout.check()?;
        Ok(layout)
    }

    /// The saved form: pretty JSON, always the same bytes for the same layout.
    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    pub fn check(&self) -> Result<(), GraphError> {
        if self.schema_version != LAYOUT_SCHEMA_VERSION {
            return refuse(format!(
                "este layout é da versão {}, e esta do Petunia lê a {LAYOUT_SCHEMA_VERSION}; apague o arquivo para recomeçar",
                self.schema_version
            ));
        }
        if self.nodes.len() > MAX_NODES {
            return refuse(format!(
                "o layout tem {} posições; o limite é {MAX_NODES}",
                self.nodes.len()
            ));
        }
        for (id, node) in &self.nodes {
            if !node.x.is_finite() || !node.y.is_finite() {
                return refuse(format!("o nó {id} do layout está numa posição inválida"));
            }
            if node.x.abs() > MAX_COORDINATE || node.y.abs() > MAX_COORDINATE {
                return refuse(format!(
                    "o nó {id} do layout está longe demais do começo da tela"
                ));
            }
        }
        if self.groups.len() > MAX_NODES {
            return refuse(format!(
                "o layout tem {} grupos; o limite é {MAX_NODES}",
                self.groups.len()
            ));
        }
        let mut ids = std::collections::BTreeSet::new();
        let mut grouped = std::collections::BTreeSet::new();
        for group in &self.groups {
            if !ids.insert(group.id) {
                return refuse(format!("dois grupos têm o mesmo número ({})", group.id));
            }
            let label = group.label.trim();
            if label.is_empty() || label.len() > 64 {
                return refuse(format!(
                    "o grupo {} precisa de um nome de 1 a 64 letras",
                    group.id
                ));
            }
            if group.nodes.len() > MAX_NODES {
                return refuse(format!(
                    "o grupo {} tem nós demais; o limite é {MAX_NODES}",
                    group.id
                ));
            }
            for node in &group.nodes {
                if !grouped.insert(*node) {
                    return refuse(format!("o nó {node} está em mais de um grupo"));
                }
            }
        }
        Ok(())
    }

    pub fn position(&self, id: u32) -> Option<(f64, f64)> {
        self.nodes.get(&id).map(|node| (node.x, node.y))
    }

    /// Moves one node. The caller saves the file; nothing here touches the disk.
    pub fn place(&mut self, id: u32, x: f64, y: f64) -> Result<(), GraphError> {
        if !x.is_finite() || !y.is_finite() {
            return refuse("a posição precisa ser um número".to_string());
        }
        if x.abs() > MAX_COORDINATE || y.abs() > MAX_COORDINATE {
            return refuse("essa posição está longe demais do começo da tela".to_string());
        }
        self.nodes.insert(id, LayoutNode { x, y });
        Ok(())
    }

    /// Creates a group around `nodes` and returns its id. The caller checks the nodes exist.
    pub fn group(&mut self, label: &str, nodes: &[u32]) -> Result<u32, GraphError> {
        let label = label.trim();
        if label.is_empty() || label.len() > 64 {
            return refuse("o grupo precisa de um nome de 1 a 64 letras".to_string());
        }
        let mut chosen: Vec<u32> = nodes.to_vec();
        chosen.sort_unstable();
        chosen.dedup();
        if chosen.is_empty() {
            return refuse("escolha pelo menos um nó para agrupar".to_string());
        }
        for node in &chosen {
            if self.groups.iter().any(|group| group.nodes.contains(node)) {
                return refuse(format!("o nó {node} já está em outro grupo"));
            }
        }
        let id = self
            .groups
            .iter()
            .map(|group| group.id)
            .max()
            .map_or(1, |highest| highest.saturating_add(1));
        self.groups.push(LayoutGroup {
            id,
            label: label.to_string(),
            nodes: chosen,
        });
        Ok(id)
    }

    /// Removes a group; the nodes stay where they are.
    pub fn ungroup(&mut self, id: u32) -> bool {
        let before = self.groups.len();
        self.groups.retain(|group| group.id != id);
        self.groups.len() != before
    }

    pub fn group_of(&self, node: u32) -> Option<u32> {
        self.groups
            .iter()
            .find(|group| group.nodes.contains(&node))
            .map(|group| group.id)
    }

    /// Drops the positions of nodes the graph no longer has, so the file does not grow forever.
    /// Groups lose departed members and disappear when they become empty.
    pub fn prune(&mut self, document: &GraphDocument) {
        self.nodes.retain(|id, _| document.node(*id).is_some());
        for group in &mut self.groups {
            group.nodes.retain(|id| document.node(*id).is_some());
        }
        self.groups.retain(|group| !group.nodes.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GraphNode;

    #[test]
    fn a_layout_saves_reads_and_keeps_the_same_bytes() {
        let mut layout = GraphLayout::new();
        layout.place(1, 40.0, 80.0).expect("place");
        layout.place(7, -12.5, 0.0).expect("place");
        let text = layout.to_text();
        assert_eq!(GraphLayout::parse(&text).expect("reads"), layout);
        assert_eq!(GraphLayout::parse(&text).expect("reads").to_text(), text);
        assert_eq!(layout.position(7), Some((-12.5, 0.0)));
        assert_eq!(layout.position(9), None);
    }

    #[test]
    fn hostile_layouts_are_refused() {
        assert!(GraphLayout::parse("{ nao e json").is_err());
        assert!(GraphLayout::parse(r#"{"schema_version":99,"nodes":{}}"#).is_err());
        assert!(
            GraphLayout::parse(r#"{"schema_version":1,"nodes":{"1":{"x":1e309,"y":0}}}"#).is_err(),
            "infinite coordinates"
        );
        assert!(
            GraphLayout::parse(r#"{"schema_version":1,"nodes":{"1":{"x":999999999.0,"y":0}}}"#)
                .is_err(),
            "too far from the origin"
        );
        let mut layout = GraphLayout::new();
        assert!(layout.place(1, f64::NAN, 0.0).is_err());
        assert!(layout.place(1, 0.0, MAX_COORDINATE * 2.0).is_err());
    }

    #[test]
    fn groups_are_validated_and_survive_a_round_trip() {
        let mut layout = GraphLayout::new();
        layout.place(1, 0.0, 0.0).expect("place");
        layout.place(2, 10.0, 0.0).expect("place");
        let id = layout.group("Porta", &[2, 1]).expect("group");
        assert_eq!(id, 1);
        assert_eq!(layout.group_of(1), Some(1));
        assert!(
            layout.group("Outro", &[1]).is_err(),
            "a node cannot be in two groups"
        );
        assert!(layout.group("", &[]).is_err());
        let text = layout.to_text();
        assert_eq!(GraphLayout::parse(&text).expect("reads"), layout);
        assert!(layout.ungroup(1));
        assert_eq!(layout.group_of(1), None);
    }

    #[test]
    fn positions_of_removed_nodes_are_pruned() {
        let mut document = GraphDocument::new();
        document.nodes.push(GraphNode {
            id: 1,
            kind: "event.on_start".to_string(),
            params: BTreeMap::new(),
        });
        let mut layout = GraphLayout::new();
        layout.place(1, 0.0, 0.0).expect("place");
        layout.place(2, 10.0, 10.0).expect("place");
        layout.prune(&document);
        assert!(layout.position(1).is_some());
        assert!(layout.position(2).is_none());
    }
}

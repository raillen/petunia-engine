//! The graph document: what is saved in `scripts/<name>.graph.json`.
//!
//! A graph is plain data: nodes with a stable id, a kind and parameters, plus edges between pins.
//! Where the nodes sit on the canvas is **not** here (it lives in a file beside this one), so a
//! change of layout never hides a change of logic in a diff. Files are hostile input: size, node
//! count, edge count and text lengths are limited before anything else looks at them.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_FILE_BYTES: usize = 1 << 20;
pub const MAX_NODES: usize = 512;
pub const MAX_EDGES: usize = 2048;
/// Longest kind, pin or parameter name.
pub const MAX_NAME_BYTES: usize = 64;
/// Longest text constant.
pub const MAX_TEXT_BYTES: usize = 4096;

/// One graph. Field order is the order of the saved file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphDocument {
    pub schema_version: u32,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// A node: `kind` names an entry of the catalog; `params` hold constants typed on an input pin and
/// settings such as an operator. `BTreeMap` keeps the saved order stable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    pub id: u32,
    pub kind: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, Json>,
}

/// A wire from an output pin to an input pin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphEdge {
    pub from: PinRef,
    pub to: PinRef,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinRef {
    pub node: u32,
    pub pin: String,
}

/// Why a file was refused, in a sentence the person can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphError(pub String);

impl std::fmt::Display for GraphError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GraphError {}

fn refuse<T>(message: impl Into<String>) -> Result<T, GraphError> {
    Err(GraphError(message.into()))
}

impl Default for GraphDocument {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphDocument {
    pub fn new() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Reads a saved graph. Checks the shape and the limits, not the meaning: a node whose kind the
    /// catalog no longer has is kept (it becomes a "ghost" the compiler reports) so opening and
    /// saving never silently deletes someone's work.
    pub fn parse(text: &str) -> Result<Self, GraphError> {
        if text.len() > MAX_FILE_BYTES {
            return refuse(format!(
                "o grafo tem mais de {} KB; divida a lógica em dois grafos",
                MAX_FILE_BYTES / 1024
            ));
        }
        let document: GraphDocument = serde_json::from_str(text)
            .map_err(|error| GraphError(format!("o arquivo do grafo não é válido: {error}")))?;
        document.check_shape()?;
        Ok(document)
    }

    /// The saved form: pretty JSON, always the same bytes for the same graph.
    pub fn to_text(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// The smallest id not used yet above every existing one.
    pub fn next_id(&self) -> u32 {
        self.nodes
            .iter()
            .map(|node| node.id)
            .max()
            .map_or(1, |highest| highest.saturating_add(1))
    }

    pub fn node(&self, id: u32) -> Option<&GraphNode> {
        self.nodes.iter().find(|node| node.id == id)
    }

    /// Limits and structure that do not need the catalog.
    pub fn check_shape(&self) -> Result<(), GraphError> {
        if self.schema_version != SCHEMA_VERSION {
            return refuse(if self.schema_version > SCHEMA_VERSION {
                format!(
                    "este grafo é da versão {}, mais nova que a deste Petunia ({SCHEMA_VERSION}); atualize o editor",
                    self.schema_version
                )
            } else {
                format!(
                    "este grafo é da versão {}, que não é mais lida (versão atual: {SCHEMA_VERSION})",
                    self.schema_version
                )
            });
        }
        if self.nodes.len() > MAX_NODES {
            return refuse(format!(
                "o grafo tem {} nós; o limite é {MAX_NODES}. Divida a lógica em dois grafos",
                self.nodes.len()
            ));
        }
        if self.edges.len() > MAX_EDGES {
            return refuse(format!(
                "o grafo tem {} ligações; o limite é {MAX_EDGES}",
                self.edges.len()
            ));
        }
        let mut seen = BTreeSet::new();
        for node in &self.nodes {
            if !seen.insert(node.id) {
                return refuse(format!("dois nós têm o mesmo número ({})", node.id));
            }
            check_name("o tipo do nó", &node.kind)?;
            for (name, value) in &node.params {
                check_name("o nome de um parâmetro", name)?;
                check_value(node.id, name, value)?;
            }
        }
        for edge in &self.edges {
            for end in [&edge.from, &edge.to] {
                if !seen.contains(&end.node) {
                    return refuse(format!(
                        "uma ligação aponta para o nó {}, que não existe",
                        end.node
                    ));
                }
                check_name("o nome de um pino", &end.pin)?;
            }
        }
        Ok(())
    }
}

fn check_name(what: &str, name: &str) -> Result<(), GraphError> {
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return refuse(format!("{what} precisa ter de 1 a {MAX_NAME_BYTES} letras"));
    }
    Ok(())
}

/// A parameter is a scalar: text, number, yes/no or none. Lists and objects would let a file hide
/// arbitrarily large data in a node.
fn check_value(node: u32, name: &str, value: &Json) -> Result<(), GraphError> {
    match value {
        Json::Null | Json::Bool(_) => Ok(()),
        Json::Number(number) => {
            if number.as_f64().is_some_and(f64::is_finite) {
                Ok(())
            } else {
                refuse(format!(
                    "o nó {node}, parâmetro '{name}': o número não é finito"
                ))
            }
        }
        Json::String(text) => {
            if text.len() > MAX_TEXT_BYTES {
                refuse(format!(
                    "o nó {node}, parâmetro '{name}': o texto passa de {MAX_TEXT_BYTES} letras"
                ))
            } else {
                Ok(())
            }
        }
        Json::Array(_) | Json::Object(_) => refuse(format!(
            "o nó {node}, parâmetro '{name}': só texto, número, sim/não ou nada cabem aqui"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GraphDocument {
        let mut document = GraphDocument::new();
        document.nodes.push(GraphNode {
            id: 4,
            kind: "event.on_start".to_string(),
            params: BTreeMap::new(),
        });
        document.nodes.push(GraphNode {
            id: 9,
            kind: "api.log.info".to_string(),
            params: BTreeMap::from([("message".to_string(), Json::from("oi"))]),
        });
        document.edges.push(GraphEdge {
            from: PinRef {
                node: 4,
                pin: "then".to_string(),
            },
            to: PinRef {
                node: 9,
                pin: "in".to_string(),
            },
        });
        document
    }

    #[test]
    fn saving_and_reading_gives_the_same_graph_and_the_same_bytes() {
        let document = sample();
        let text = document.to_text();
        let again = GraphDocument::parse(&text).expect("reads");
        assert_eq!(again, document);
        assert_eq!(again.to_text(), text);
        assert_eq!(document.next_id(), 10);
        assert_eq!(GraphDocument::new().next_id(), 1);
    }

    #[test]
    fn a_wire_to_a_missing_node_or_a_repeated_id_is_refused() {
        let mut document = sample();
        document.edges.push(GraphEdge {
            from: PinRef {
                node: 4,
                pin: "then".to_string(),
            },
            to: PinRef {
                node: 77,
                pin: "in".to_string(),
            },
        });
        assert!(document.check_shape().is_err());
        let mut document = sample();
        document.nodes.push(document.nodes[0].clone());
        assert!(document.check_shape().is_err());
    }

    #[test]
    fn limits_and_versions_are_enforced() {
        let mut document = sample();
        document.schema_version = SCHEMA_VERSION + 1;
        let message = document.check_shape().expect_err("newer").0;
        assert!(message.contains("mais nova"), "{message}");

        let mut document = GraphDocument::new();
        for id in 0..=MAX_NODES as u32 {
            document.nodes.push(GraphNode {
                id,
                kind: "flow.if".to_string(),
                params: BTreeMap::new(),
            });
        }
        assert!(document.check_shape().is_err());

        let mut document = sample();
        document.nodes[1].params.insert(
            "message".to_string(),
            Json::from("x".repeat(MAX_TEXT_BYTES + 1)),
        );
        assert!(document.check_shape().is_err());
    }
}

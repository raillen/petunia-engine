//! Editing a graph: add a node into a chain, remove one without breaking the chain, connect,
//! disconnect, set a constant. Each edit checks the catalog first and refuses with the reason, so a
//! person (or a script, or an agent) never leaves the file in a state the compiler cannot explain.
//! The functions only change the document; undo is the caller's (it keeps snapshots).

use serde_json::Value as Json;

use crate::catalog::{Catalog, Direction, Lowering, NodeClass, NodeSpec, PinKind, SettingChoices};
use crate::compile::literal;
use crate::model::{
    GraphDocument, GraphEdge, GraphError, GraphNode, MAX_EDGES, MAX_NAME_BYTES, MAX_NODES, PinRef,
};

fn refuse<T>(message: impl Into<String>) -> Result<T, GraphError> {
    Err(GraphError(message.into()))
}

/// The pin a chain continues from: `then` for actions and events, `done` for nodes that branch
/// (an `if` has a `then` branch, but the chain goes on from `done`).
pub fn continuation(spec: &NodeSpec) -> Option<&str> {
    let name = if spec.class == NodeClass::Flow {
        "done"
    } else {
        "then"
    };
    spec.pin(name, Direction::Out)
        .filter(|pin| pin.kind == PinKind::Exec)
        .map(|pin| pin.name.as_str())
}

fn spec_of<'a>(
    document: &GraphDocument,
    catalog: &'a Catalog,
    id: u32,
) -> Result<&'a NodeSpec, GraphError> {
    let node = document
        .node(id)
        .ok_or_else(|| GraphError(format!("o nó {id} não existe")))?;
    catalog.get(&node.kind).ok_or_else(|| {
        GraphError(format!(
            "o nó {id} usa '{}', que não existe mais; só dá para apagá-lo",
            node.kind
        ))
    })
}

fn exec_edge_from<'a>(
    document: &'a GraphDocument,
    catalog: &Catalog,
    from: &PinRef,
) -> Option<&'a GraphEdge> {
    document.edges.iter().find(|edge| {
        edge.from == *from
            && spec_of(document, catalog, edge.from.node)
                .ok()
                .and_then(|spec| spec.pin(&edge.from.pin, Direction::Out))
                .is_some_and(|pin| pin.kind == PinKind::Exec)
    })
}

/// The wire that makes `id` run (an edge into its `in` pin), if any.
fn exec_edge_into(document: &GraphDocument, id: u32) -> Option<&GraphEdge> {
    document
        .edges
        .iter()
        .find(|edge| edge.to.node == id && edge.to.pin == "in")
}

/// Adds a node. With `after` (an execution output), the node goes into that chain right there:
/// whatever ran next now runs after the new node.
pub fn add_node(
    document: &mut GraphDocument,
    catalog: &Catalog,
    kind: &str,
    after: Option<&PinRef>,
) -> Result<u32, GraphError> {
    let spec = catalog
        .get(kind)
        .ok_or_else(|| GraphError(format!("não existe o nó '{kind}'")))?;
    if document.nodes.len() >= MAX_NODES {
        return refuse(format!(
            "o grafo já tem {MAX_NODES} nós; divida a lógica em dois grafos"
        ));
    }
    if spec.class == NodeClass::Event && document.nodes.iter().any(|node| node.kind == kind) {
        return refuse(format!(
            "o evento '{}' já está neste grafo; cada evento só pode aparecer uma vez",
            spec.title
        ));
    }
    let id = document.next_id();
    let mut splice: Option<(PinRef, Option<PinRef>)> = None;
    if let Some(anchor) = after {
        let anchor_spec = spec_of(document, catalog, anchor.node)?;
        anchor_spec
            .pin(&anchor.pin, Direction::Out)
            .filter(|pin| pin.kind == PinKind::Exec)
            .ok_or_else(|| {
                GraphError(format!(
                    "'{}' não é uma saída de execução do nó {}",
                    anchor.pin, anchor.node
                ))
            })?;
        if !spec.has_exec_in() {
            return refuse(format!(
                "'{}' não entra numa sequência: ele só calcula um valor ou começa um caminho",
                spec.title
            ));
        }
        if document.edges.len() + 2 > MAX_EDGES {
            return refuse("o grafo já tem ligações demais".to_string());
        }
        let previous_next = exec_edge_from(document, catalog, anchor).map(|edge| edge.to.clone());
        splice = Some((anchor.clone(), previous_next));
    }
    document.nodes.push(GraphNode {
        id,
        kind: kind.to_string(),
        params: std::collections::BTreeMap::new(),
    });
    if let Some((anchor, previous_next)) = splice {
        document
            .edges
            .retain(|edge| !(edge.from == anchor && previous_next.as_ref() == Some(&edge.to)));
        document.edges.push(GraphEdge {
            from: anchor,
            to: PinRef {
                node: id,
                pin: "in".to_string(),
            },
        });
        if let (Some(next), Some(pin)) = (previous_next, continuation(spec)) {
            document.edges.push(GraphEdge {
                from: PinRef {
                    node: id,
                    pin: pin.to_string(),
                },
                to: next,
            });
        }
    }
    Ok(id)
}

/// Removes a node and its wires. A node in the middle of a chain is cut out and the chain closes
/// over the gap; the branches of a removed `if` or loop are left loose, not deleted.
pub fn remove_node(
    document: &mut GraphDocument,
    catalog: &Catalog,
    id: u32,
) -> Result<(), GraphError> {
    if document.node(id).is_none() {
        return refuse(format!("o nó {id} não existe"));
    }
    let kind = document
        .node(id)
        .map(|node| node.kind.clone())
        .unwrap_or_default();
    let spec = catalog.get(&kind);
    let previous = exec_edge_into(document, id).map(|edge| edge.from.clone());
    let next = spec.and_then(continuation).and_then(|pin| {
        exec_edge_from(
            document,
            catalog,
            &PinRef {
                node: id,
                pin: pin.to_string(),
            },
        )
        .map(|edge| edge.to.clone())
    });
    document
        .edges
        .retain(|edge| edge.from.node != id && edge.to.node != id);
    document.nodes.retain(|node| node.id != id);
    if let (Some(from), Some(to)) = (previous, next) {
        document.edges.push(GraphEdge { from, to });
    }
    Ok(())
}

/// Whether running `target` could lead back to `start` (walking the execution wires backwards from
/// `start` reaches `target`).
fn runs_before(document: &GraphDocument, target: u32, start: u32) -> bool {
    let mut current = start;
    for _ in 0..=document.nodes.len() {
        if current == target {
            return true;
        }
        match exec_edge_into(document, current) {
            Some(edge) => current = edge.from.node,
            None => return false,
        }
    }
    true
}

/// Connects an output pin to an input pin.
/// - data: the old wire on that input (and its constant) is replaced;
/// - execution: both pins must be free, and the wire cannot make execution loop back.
pub fn connect(
    document: &mut GraphDocument,
    catalog: &Catalog,
    from: &PinRef,
    to: &PinRef,
) -> Result<(), GraphError> {
    if from.node == to.node {
        return refuse("não dá para ligar um nó nele mesmo".to_string());
    }
    let from_spec = spec_of(document, catalog, from.node)?;
    let to_spec = spec_of(document, catalog, to.node)?;
    let from_pin = from_spec.pin(&from.pin, Direction::Out).ok_or_else(|| {
        GraphError(format!(
            "o nó {} ({}) não tem a saída '{}'",
            from.node, from_spec.title, from.pin
        ))
    })?;
    let to_pin = to_spec.pin(&to.pin, Direction::In).ok_or_else(|| {
        GraphError(format!(
            "o nó {} ({}) não tem a entrada '{}'",
            to.node, to_spec.title, to.pin
        ))
    })?;
    if document.edges.len() >= MAX_EDGES {
        return refuse(format!("o grafo já tem {MAX_EDGES} ligações"));
    }
    match (from_pin.kind, to_pin.kind) {
        (PinKind::Exec, PinKind::Exec) => {
            if exec_edge_from(document, catalog, from).is_some() {
                return refuse(format!(
                    "a saída '{}' já leva a outro nó; para rodar mais uma coisa, adicione um nó depois dela",
                    from_pin.label
                ));
            }
            if exec_edge_into(document, to.node).is_some() {
                return refuse(format!(
                    "o nó {} ({}) já roda depois de outro nó; cada nó roda depois de um só",
                    to.node, to_spec.title
                ));
            }
            if runs_before(document, to.node, from.node) {
                return refuse(
                    "essa ligação faria a execução voltar para um nó que já rodou".to_string(),
                );
            }
            document.edges.push(GraphEdge {
                from: from.clone(),
                to: to.clone(),
            });
            Ok(())
        }
        (PinKind::Data(source), PinKind::Data(target)) => {
            if !target.accepts(source) {
                return refuse(format!(
                    "'{}' dá {} e '{}' pede {}; não combinam",
                    from_pin.label,
                    source.label(),
                    to_pin.label,
                    target.label()
                ));
            }
            document.edges.retain(|edge| edge.to != *to);
            if let Some(node) = document.nodes.iter_mut().find(|node| node.id == to.node) {
                node.params.remove(&to.pin);
            }
            document.edges.push(GraphEdge {
                from: from.clone(),
                to: to.clone(),
            });
            Ok(())
        }
        _ => refuse(format!(
            "'{}' é ordem de execução e '{}' é um valor; não se ligam",
            from_pin.label, to_pin.label
        )),
    }
}

/// Removes the wire that ends at `pin` (an input), or the one that starts at `pin` when it is an
/// execution output. Returns how many wires went away.
pub fn disconnect(document: &mut GraphDocument, pin: &PinRef) -> usize {
    let before = document.edges.len();
    document.edges.retain(|edge| {
        !(edge.to == *pin || (edge.from == *pin && is_exec_pin_name(&edge.from.pin)))
    });
    before - document.edges.len()
}

/// Execution outputs are named `then`, `else`, `done` or `body`; data outputs never are.
fn is_exec_pin_name(name: &str) -> bool {
    matches!(name, "then" | "else" | "done" | "body")
}

/// Sets (or, with `None`, clears) a constant on an input pin or a setting of a node. A constant on
/// a wired input replaces the wire: the last thing the person chose wins.
pub fn set_param(
    document: &mut GraphDocument,
    catalog: &Catalog,
    id: u32,
    name: &str,
    value: Option<Json>,
) -> Result<(), GraphError> {
    let spec = spec_of(document, catalog, id)?;
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return refuse("o nome do campo está vazio ou é grande demais".to_string());
    }
    let is_setting = spec.setting(name).is_some();
    let pin = spec.pin(name, Direction::In);
    if !is_setting && pin.is_none_or(|pin| !matches!(pin.kind, PinKind::Data(_))) {
        return refuse(format!(
            "o nó {id} ({}) não tem o campo '{name}'",
            spec.title
        ));
    }
    if let Some(value) = &value {
        if let Some(setting) = spec.setting(name) {
            let valid = match &setting.choices {
                SettingChoices::OneOf(options) => value
                    .as_str()
                    .is_some_and(|text| options.iter().any(|(option, _)| option == text)),
                SettingChoices::Text => value
                    .as_str()
                    .is_some_and(|text| !text.trim().is_empty() && text.len() <= MAX_NAME_BYTES),
            };
            if !valid {
                return refuse(format!("'{value}' não serve para '{}'", setting.label));
            }
        } else if let Some(PinKind::Data(data_type)) = pin.map(|pin| pin.kind) {
            literal(value, data_type).map_err(|reason| {
                GraphError(format!(
                    "'{}': {reason}",
                    pin.map_or(name, |p| p.label.as_str())
                ))
            })?;
        }
    }
    if pin.is_some() && value.is_some() {
        let target = PinRef {
            node: id,
            pin: name.to_string(),
        };
        document.edges.retain(|edge| edge.to != target);
    }
    if let Some(node) = document.nodes.iter_mut().find(|node| node.id == id) {
        match value {
            Some(value) => {
                node.params.insert(name.to_string(), value);
            }
            None => {
                node.params.remove(name);
            }
        }
    }
    Ok(())
}

/// Whether a node's kind is an event (chains start at events).
pub fn is_event(spec: &NodeSpec) -> bool {
    matches!(spec.lowering, Lowering::Event { .. })
}

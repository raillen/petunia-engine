//! The graph as a readable outline, and the questions an editor asks about it: what can I add
//! here, what can I wire to this input.
//!
//! The outline is the **accessible view**: a tree of "WHEN … DO …" rows a keyboard and a screen
//! reader can walk, and the node canvas draws the same graph. Building it here (not in the GUI)
//! keeps the GUI thin and lets a script or an agent read a graph the same way a person does.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value as Json, json};

use crate::catalog::{
    Catalog, DataType, Direction, NodeClass, NodeSpec, PinDefault, PinKind, SettingChoices,
};
use crate::compile::{Diagnostic, Severity, check, literal};
use crate::edit::{continuation, is_event};
use crate::model::{GraphDocument, GraphNode, PinRef};

/// How deep an expression is described before it is cut with "…".
const MAX_DESCRIBE_DEPTH: usize = 5;

fn type_name(kind: PinKind) -> &'static str {
    match kind {
        PinKind::Exec => "Execução",
        PinKind::Data(data_type) => data_type.label(),
    }
}

/// One catalog entry for a palette.
pub fn spec_json(spec: &NodeSpec) -> Json {
    json!({
        "kind": spec.kind,
        "title": spec.title,
        "category": spec.category,
        "summary": spec.summary,
        "class": class_name(spec.class),
        "permission": spec.permission,
        "pins": spec.pins.iter().map(|pin| json!({
            "name": pin.name,
            "label": pin.label,
            "direction": if pin.direction == Direction::In { "in" } else { "out" },
            "type": type_name(pin.kind),
        })).collect::<Vec<_>>(),
    })
}

fn class_name(class: NodeClass) -> &'static str {
    match class {
        NodeClass::Event => "event",
        NodeClass::Action => "action",
        NodeClass::Value => "value",
        NodeClass::Flow => "flow",
    }
}

/// What the palette is looking for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Want {
    /// An action or a flow node, to put in a chain.
    Chain,
    /// An event, to start a chain.
    Event,
    /// A node whose result fits an input of this type.
    Value(DataType),
    Any,
}

/// Lower case, without accents, so "posicao" finds "Posição".
pub fn fold(text: &str) -> String {
    text.chars()
        .map(|character| {
            let lower = character.to_lowercase().next().unwrap_or(character);
            match lower {
                'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
                'é' | 'è' | 'ê' | 'ë' => 'e',
                'í' | 'ì' | 'î' | 'ï' => 'i',
                'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
                'ú' | 'ù' | 'û' | 'ü' => 'u',
                'ç' => 'c',
                other => other,
            }
        })
        .collect()
}

fn fits(spec: &NodeSpec, want: Want) -> bool {
    match want {
        Want::Any => true,
        Want::Chain => matches!(spec.class, NodeClass::Action | NodeClass::Flow),
        Want::Event => spec.class == NodeClass::Event,
        Want::Value(target) => {
            spec.class == NodeClass::Value
                && spec.pins.iter().any(|pin| {
                    pin.direction == Direction::Out
                        && matches!(pin.kind, PinKind::Data(source) if target.accepts(source))
                })
        }
    }
}

/// Nodes matching `query` (any order of words, accents ignored), best first. An empty query lists
/// everything that fits, by category and title.
pub fn search<'a>(catalog: &'a Catalog, query: &str, want: Want) -> Vec<&'a NodeSpec> {
    let words: Vec<String> = fold(query).split_whitespace().map(str::to_string).collect();
    let mut found: Vec<(u8, &NodeSpec)> = Vec::new();
    for spec in catalog.iter().filter(|spec| fits(spec, want)) {
        let title = fold(&spec.title);
        let category = fold(&spec.category);
        let summary = fold(&spec.summary);
        let kind = fold(&spec.kind);
        let mut worst = 0u8;
        let mut matched = true;
        for word in &words {
            let rank = if title.starts_with(word.as_str()) {
                0
            } else if title.contains(word.as_str()) {
                1
            } else if category.contains(word.as_str()) {
                2
            } else if summary.contains(word.as_str()) {
                3
            } else if kind.contains(word.as_str()) {
                4
            } else {
                matched = false;
                break;
            };
            worst = worst.max(rank);
        }
        if matched {
            found.push((worst, spec));
        }
    }
    found.sort_by(|a, b| {
        (a.0, a.1.category.as_str(), a.1.title.as_str()).cmp(&(
            b.0,
            b.1.category.as_str(),
            b.1.title.as_str(),
        ))
    });
    found.into_iter().map(|(_, spec)| spec).collect()
}

struct Outliner<'a> {
    document: &'a GraphDocument,
    catalog: &'a Catalog,
    diagnostics: Vec<Diagnostic>,
    exec_next: BTreeMap<(u32, String), u32>,
    exec_prev: BTreeMap<u32, PinRef>,
    data_in: BTreeMap<(u32, String), PinRef>,
    reached: BTreeSet<u32>,
}

impl<'a> Outliner<'a> {
    fn new(document: &'a GraphDocument, catalog: &'a Catalog) -> Self {
        let mut exec_next = BTreeMap::new();
        let mut exec_prev = BTreeMap::new();
        let mut data_in = BTreeMap::new();
        for edge in &document.edges {
            let kind = document
                .node(edge.from.node)
                .and_then(|node| catalog.get(&node.kind))
                .and_then(|spec| spec.pin(&edge.from.pin, Direction::Out))
                .map(|pin| pin.kind);
            match kind {
                Some(PinKind::Exec) => {
                    exec_next
                        .entry((edge.from.node, edge.from.pin.clone()))
                        .or_insert(edge.to.node);
                    exec_prev
                        .entry(edge.to.node)
                        .or_insert_with(|| edge.from.clone());
                }
                Some(PinKind::Data(_)) => {
                    data_in
                        .entry((edge.to.node, edge.to.pin.clone()))
                        .or_insert_with(|| edge.from.clone());
                }
                None => {}
            }
        }
        Self {
            document,
            catalog,
            diagnostics: check(document, catalog).diagnostics,
            exec_next,
            exec_prev,
            data_in,
            reached: BTreeSet::new(),
        }
    }

    fn title_of(&self, node: &GraphNode) -> String {
        self.catalog
            .get(&node.kind)
            .map_or_else(|| node.kind.clone(), |spec| spec.title.clone())
    }

    fn problems(&self, id: u32) -> Vec<Json> {
        self.diagnostics
            .iter()
            .filter(|finding| finding.node == Some(id))
            .map(|finding| {
                json!({
                    "severity": if finding.severity == Severity::Error { "error" } else { "warning" },
                    "pin": finding.pin,
                    "code": finding.code,
                    "message": finding.message,
                })
            })
            .collect()
    }

    /// Settings of a node with the value now in effect and its label.
    fn settings(&self, node: &GraphNode, spec: &NodeSpec) -> Vec<Json> {
        spec.settings
            .iter()
            .map(|setting| {
                let given = node.params.get(&setting.name).and_then(Json::as_str);
                match &setting.choices {
                    SettingChoices::OneOf(options) => {
                        let current = given
                            .and_then(|value| options.iter().find(|(option, _)| option == value))
                            .or_else(|| options.first());
                        json!({
                            "name": setting.name,
                            "label": setting.label,
                            "value": current.map(|(value, _)| value.as_str()),
                            "value_label": current.map(|(_, label)| label.as_str()),
                            "choices": options.iter().map(|(value, label)| json!({ "value": value, "label": label })).collect::<Vec<_>>(),
                        })
                    }
                    SettingChoices::Text => json!({
                        "name": setting.name,
                        "label": setting.label,
                        "value": given.unwrap_or(""),
                        "value_label": given.unwrap_or(""),
                        "choices": Json::Null,
                    }),
                }
            })
            .collect()
    }

    /// An input pin: where its value comes from, in words, plus the wired node when there is one.
    fn input(
        &mut self,
        node: &GraphNode,
        spec: &NodeSpec,
        pin_name: &str,
        depth: usize,
    ) -> Option<Json> {
        let pin = spec.pin(pin_name, Direction::In)?;
        let PinKind::Data(data_type) = pin.kind else {
            return None;
        };
        let wire = self.data_in.get(&(node.id, pin_name.to_string())).cloned();
        let (source, text, constant, nested, from) = if let Some(wire) = wire {
            match self.document.node(wire.node) {
                Some(source_node) => {
                    let source_spec = self.catalog.get(&source_node.kind);
                    let is_value = source_spec.is_some_and(|spec| spec.class == NodeClass::Value);
                    if is_value {
                        let nested = self.step(source_node.id, depth + 1);
                        let text = self.describe_value(source_node.id, depth + 1);
                        ("wire", text, Json::Null, nested, Json::Null)
                    } else {
                        let label = source_spec
                            .and_then(|spec| spec.pin(&wire.pin, Direction::Out))
                            .map_or(wire.pin.clone(), |pin| pin.label.clone());
                        let text = format!("{label} ({})", self.title_of(source_node));
                        (
                            "wire",
                            text,
                            Json::Null,
                            Json::Null,
                            json!({ "node": wire.node, "pin": wire.pin }),
                        )
                    }
                }
                None => (
                    "missing",
                    "falta".to_string(),
                    Json::Null,
                    Json::Null,
                    Json::Null,
                ),
            }
        } else if let Some(value) = node.params.get(pin_name) {
            let text = literal(value, data_type).unwrap_or_else(|_| value.to_string());
            (
                "constant",
                text.trim_matches('"').to_string(),
                value.clone(),
                Json::Null,
                Json::Null,
            )
        } else if pin.default == PinDefault::ThisObject {
            (
                "default",
                "este objeto".to_string(),
                Json::Null,
                Json::Null,
                Json::Null,
            )
        } else {
            (
                "missing",
                "falta".to_string(),
                Json::Null,
                Json::Null,
                Json::Null,
            )
        };
        Some(json!({
            "pin": pin.name,
            "label": pin.label,
            "type": data_type.label(),
            "source": source,
            "text": text,
            "constant": constant,
            "node": nested,
            "from": from,
        }))
    }

    /// A pure node as one phrase: `Comparar é menor que: A = Ler variável 'x'; B = 3`.
    fn describe_value(&self, id: u32, depth: usize) -> String {
        let Some(node) = self.document.node(id) else {
            return "?".to_string();
        };
        let Some(spec) = self.catalog.get(&node.kind) else {
            return format!("nó desconhecido ({})", node.kind);
        };
        if depth > MAX_DESCRIBE_DEPTH {
            return "…".to_string();
        }
        let mut text = spec.title.clone();
        for setting in &spec.settings {
            let shown = match (
                &setting.choices,
                node.params.get(&setting.name).and_then(Json::as_str),
            ) {
                (SettingChoices::OneOf(options), value) => value
                    .and_then(|value| options.iter().find(|(option, _)| option == value))
                    .or_else(|| options.first())
                    .map(|(_, label)| label.clone()),
                (SettingChoices::Text, Some(value)) => Some(format!("'{value}'")),
                (SettingChoices::Text, None) => None,
            };
            if let Some(shown) = shown {
                text.push(' ');
                text.push_str(&shown);
            }
        }
        let mut arguments = Vec::new();
        for pin in spec
            .pins
            .iter()
            .filter(|pin| pin.direction == Direction::In && matches!(pin.kind, PinKind::Data(_)))
        {
            let PinKind::Data(data_type) = pin.kind else {
                continue;
            };
            let value = if let Some(wire) = self.data_in.get(&(id, pin.name.clone())) {
                match self.document.node(wire.node) {
                    Some(source)
                        if self
                            .catalog
                            .get(&source.kind)
                            .is_some_and(|s| s.class == NodeClass::Value) =>
                    {
                        self.describe_value(source.id, depth + 1)
                    }
                    Some(source) => {
                        let label = self
                            .catalog
                            .get(&source.kind)
                            .and_then(|spec| spec.pin(&wire.pin, Direction::Out))
                            .map_or(wire.pin.clone(), |pin| pin.label.clone());
                        format!("{label} ({})", self.title_of(source))
                    }
                    None => "falta".to_string(),
                }
            } else if let Some(value) = node.params.get(&pin.name) {
                literal(value, data_type)
                    .unwrap_or_else(|_| value.to_string())
                    .trim_matches('"')
                    .to_string()
            } else if pin.default == PinDefault::ThisObject {
                "este objeto".to_string()
            } else {
                "falta".to_string()
            };
            arguments.push(format!("{} = {value}", pin.label));
        }
        if !arguments.is_empty() {
            text.push_str(": ");
            text.push_str(&arguments.join("; "));
        }
        text
    }

    /// One node as a step; chains are followed by [`Outliner::chain`].
    fn step(&mut self, id: u32, depth: usize) -> Json {
        let Some(node) = self.document.node(id) else {
            return Json::Null;
        };
        let Some(spec) = self.catalog.get(&node.kind) else {
            return json!({
                "node": id,
                "kind": node.kind,
                "title": node.kind,
                "class": "ghost",
                "text": format!("Nó desconhecido: {}", node.kind),
                "inputs": [],
                "settings": [],
                "branches": [],
                "after": Json::Null,
                "problems": self.problems(id),
            });
        };
        if depth > MAX_DESCRIBE_DEPTH + 2 {
            return Json::Null;
        }
        self.reached.insert(id);
        let mut inputs = Vec::new();
        for pin in spec
            .pins
            .iter()
            .filter(|pin| pin.direction == Direction::In && matches!(pin.kind, PinKind::Data(_)))
        {
            if let Some(input) = self.input(node, spec, &pin.name, depth) {
                inputs.push(input);
            }
        }
        let shown: Vec<String> = inputs
            .iter()
            .filter(|input| input["source"] != "default")
            .map(|input| {
                format!(
                    "{}: {}",
                    input["label"].as_str().unwrap_or(""),
                    input["text"].as_str().unwrap_or("")
                )
            })
            .collect();
        let mut settings_text = String::new();
        for setting in self.settings(node, spec) {
            let value = setting
                .get("value_label")
                .and_then(Json::as_str)
                .unwrap_or("");
            if !value.is_empty() {
                settings_text.push(' ');
                settings_text.push_str(value);
            }
        }
        let text = match (spec.class, spec.kind.as_str()) {
            (NodeClass::Event, _) => format!("QUANDO {}", spec.title),
            (NodeClass::Flow, "flow.if") => format!(
                "SE {}",
                shown
                    .first()
                    .map_or("falta", |text| text.trim_start_matches("Condição: "))
            ),
            (NodeClass::Flow, "flow.repeat") => format!(
                "REPETIR {} VEZES",
                shown
                    .first()
                    .map_or("falta", |text| text.trim_start_matches("Quantas vezes: "))
            ),
            (NodeClass::Flow, "flow.each") => format!(
                "PARA CADA ITEM EM {}",
                shown
                    .first()
                    .map_or("falta", |text| text.trim_start_matches("Lista: "))
            ),
            _ if shown.is_empty() => format!("{}{settings_text}", spec.title),
            _ => format!("{}{settings_text} — {}", spec.title, shown.join("; ")),
        };
        let mut branches = Vec::new();
        for pin in spec.pins.iter().filter(|pin| {
            pin.direction == Direction::Out
                && pin.kind == PinKind::Exec
                && Some(pin.name.as_str()) != continuation(spec)
        }) {
            let first = self.exec_next.get(&(id, pin.name.clone())).copied();
            let steps = first.map_or_else(Vec::new, |first| self.chain(first, depth + 1));
            branches.push(json!({
                "pin": pin.name,
                "label": pin.label,
                "after": { "node": id, "pin": pin.name },
                "steps": steps,
            }));
        }
        let after = if spec.class == NodeClass::Value {
            Json::Null
        } else {
            continuation(spec).map_or(Json::Null, |pin| json!({ "node": id, "pin": pin }))
        };
        json!({
            "node": id,
            "kind": spec.kind,
            "title": spec.title,
            "class": class_name(spec.class),
            "text": text,
            "summary": spec.summary,
            "inputs": inputs,
            "settings": self.settings(node, spec),
            "branches": branches,
            "after": after,
            "problems": self.problems(id),
        })
    }

    /// A chain: this node, then what runs after it (branches nest inside their node).
    fn chain(&mut self, start: u32, depth: usize) -> Vec<Json> {
        let mut steps = Vec::new();
        let mut current = Some(start);
        let mut seen = BTreeSet::new();
        while let Some(id) = current {
            if !seen.insert(id) || steps.len() > self.document.nodes.len() {
                break;
            }
            let step = self.step(id, depth);
            if step.is_null() {
                break;
            }
            let next = self
                .document
                .node(id)
                .and_then(|node| self.catalog.get(&node.kind))
                .and_then(continuation)
                .and_then(|pin| self.exec_next.get(&(id, pin.to_string())).copied());
            steps.push(step);
            current = next;
        }
        steps
    }
}

/// The whole graph as an outline: events with their chains, loose parts, and every finding.
pub fn outline(document: &GraphDocument, catalog: &Catalog) -> Json {
    let mut outliner = Outliner::new(document, catalog);
    let mut events: Vec<&GraphNode> = document
        .nodes
        .iter()
        .filter(|node| catalog.get(&node.kind).is_some_and(is_event))
        .collect();
    events.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.id.cmp(&b.id)));
    let mut event_rows = Vec::new();
    for node in events {
        let mut step = outliner.step(node.id, 0);
        let chain = outliner
            .exec_next
            .get(&(node.id, "then".to_string()))
            .copied()
            .map_or_else(Vec::new, |first| outliner.chain(first, 1));
        if let Some(object) = step.as_object_mut() {
            object.insert("steps".to_string(), Json::Array(chain));
        }
        event_rows.push(step);
    }
    // Loose: chains that start at a node nobody runs, and values nothing reads.
    let mut loose = Vec::new();
    let mut loose_values = Vec::new();
    let ids: Vec<u32> = document.nodes.iter().map(|node| node.id).collect();
    for id in ids {
        let Some(spec) = document.node(id).and_then(|node| catalog.get(&node.kind)) else {
            loose_values.push(outliner.step(id, 0));
            continue;
        };
        match spec.class {
            NodeClass::Action | NodeClass::Flow
                if !outliner.reached.contains(&id) && !outliner.exec_prev.contains_key(&id) =>
            {
                loose.push(json!({ "steps": outliner.chain(id, 0) }));
            }
            NodeClass::Value => {
                let read = document.edges.iter().any(|edge| edge.from.node == id);
                if !read {
                    loose_values.push(outliner.step(id, 0));
                }
            }
            _ => {}
        }
    }
    let diagnostics: Vec<Json> = outliner
        .diagnostics
        .iter()
        .map(|finding| {
            json!({
                "severity": if finding.severity == Severity::Error { "error" } else { "warning" },
                "node": finding.node,
                "pin": finding.pin,
                "code": finding.code,
                "message": finding.message,
            })
        })
        .collect();
    json!({
        "events": event_rows,
        "loose": loose,
        "loose_values": loose_values,
        "diagnostics": diagnostics,
        "nodes": document.nodes.len(),
    })
}

/// The node that decides **when** `id` runs: itself if it sits in a chain, else the first chain
/// node that reads its value.
fn runner(
    document: &GraphDocument,
    catalog: &Catalog,
    id: u32,
    seen: &mut BTreeSet<u32>,
) -> Option<u32> {
    if !seen.insert(id) {
        return None;
    }
    let spec = document.node(id).and_then(|node| catalog.get(&node.kind))?;
    if spec.has_exec_in() || is_event(spec) {
        return Some(id);
    }
    let mut readers: Vec<u32> = document
        .edges
        .iter()
        .filter(|edge| edge.from.node == id)
        .map(|edge| edge.to.node)
        .collect();
    readers.sort_unstable();
    readers
        .into_iter()
        .find_map(|reader| runner(document, catalog, reader, seen))
}

/// The outputs an input of `node` could be wired to: everything that already exists when the node
/// runs (the event's values, results of earlier actions, the index of an enclosing loop) and fits
/// the input's type. Pure nodes are not listed: add one, then wire it.
pub fn sources(document: &GraphDocument, catalog: &Catalog, node: u32, pin: &str) -> Vec<Json> {
    let Some(target_type) = document
        .node(node)
        .and_then(|found| catalog.get(&found.kind))
        .and_then(|spec| spec.pin(pin, Direction::In))
        .and_then(|pin| match pin.kind {
            PinKind::Data(data_type) => Some(data_type),
            PinKind::Exec => None,
        })
    else {
        return Vec::new();
    };
    let Some(owner) = runner(document, catalog, node, &mut BTreeSet::new()) else {
        return Vec::new();
    };
    let outliner = Outliner::new(document, catalog);
    let mut visible: Vec<(u32, String)> = Vec::new();
    let mut current = owner;
    for _ in 0..=document.nodes.len() {
        let Some(from) = outliner.exec_prev.get(&current) else {
            break;
        };
        let Some(spec) = document
            .node(from.node)
            .and_then(|found| catalog.get(&found.kind))
        else {
            break;
        };
        let data_outputs = spec
            .pins
            .iter()
            .filter(|pin| pin.direction == Direction::Out && matches!(pin.kind, PinKind::Data(_)));
        match spec.class {
            NodeClass::Event => {
                visible.extend(data_outputs.map(|pin| (from.node, pin.name.clone())));
            }
            NodeClass::Action if from.pin == "then" => {
                visible.extend(data_outputs.map(|pin| (from.node, pin.name.clone())));
            }
            NodeClass::Flow if from.pin == "body" => {
                visible.extend(data_outputs.map(|pin| (from.node, pin.name.clone())));
            }
            _ => {}
        }
        current = from.node;
    }
    visible.sort();
    visible
        .into_iter()
        .filter_map(|(source, output)| {
            let found = document.node(source)?;
            let spec = catalog.get(&found.kind)?;
            let pin = spec.pin(&output, Direction::Out)?;
            let PinKind::Data(source_type) = pin.kind else {
                return None;
            };
            target_type.accepts(source_type).then(|| {
                json!({
                    "from": { "node": source, "pin": output },
                    "label": format!("{} ({})", pin.label, spec.title),
                    "type": source_type.label(),
                })
            })
        })
        .collect()
}

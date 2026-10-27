//! Checks a graph and writes it as Aipo text.
//!
//! Compiling is deterministic: the same graph gives the same bytes, whatever order its nodes are
//! stored in. Every line records the node it came from, so an error found later (in the compiler
//! or while the game runs) can be shown on the node, not on a line of generated text.
//!
//! The rules that keep generated code structured:
//! - an execution input has at most one wire in, and an execution output at most one wire out, so a
//!   chain is a tree that maps straight onto `if` / `repeat` / `each` blocks;
//! - a data input has at most one wire in; an output may feed many inputs;
//! - a value made by an action (or a loop) can only be used **after** that node runs, on the same
//!   path; pure nodes are written inline where they are used, like a spreadsheet formula.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;

use crate::catalog::{
    Catalog, DataType, Direction, Lowering, NodeClass, NodeSpec, PinDefault, PinKind,
    SettingChoices,
};
use crate::model::{GraphDocument, GraphNode, MAX_NAME_BYTES, PinRef};

/// Most nested blocks (`if` inside `repeat` inside ...).
const MAX_DEPTH: usize = 32;
/// Most pieces of one expression; stops a diamond of shared nodes from growing without limit.
const MAX_EXPRESSION_PARTS: usize = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One finding, aimed at a node (and a pin) when it can be.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub node: Option<u32>,
    pub pin: Option<String>,
    pub code: &'static str,
    pub message: String,
}

/// A graph written as Aipo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compiled {
    pub text: String,
    /// For each line of `text` (line 1 is index 0), the node it came from, if any.
    pub line_nodes: Vec<Option<u32>>,
    pub warnings: Vec<Diagnostic>,
}

/// Compiles, or returns every error found (warnings travel inside `Compiled`).
pub fn compile(document: &GraphDocument, catalog: &Catalog) -> Result<Compiled, Vec<Diagnostic>> {
    let report = check(document, catalog);
    match report.compiled {
        Some(compiled) => Ok(compiled),
        None => Err(report.diagnostics),
    }
}

/// Everything found, errors and warnings, in one list.
pub fn validate(document: &GraphDocument, catalog: &Catalog) -> Vec<Diagnostic> {
    check(document, catalog).diagnostics
}

pub struct Report {
    /// `Some` only when there is no error.
    pub compiled: Option<Compiled>,
    pub diagnostics: Vec<Diagnostic>,
}

pub fn check(document: &GraphDocument, catalog: &Catalog) -> Report {
    let mut generator = Generator::new(document, catalog);
    generator.run();
    let has_error = generator
        .diagnostics
        .iter()
        .any(|finding| finding.severity == Severity::Error);
    let diagnostics = generator.diagnostics.clone();
    let compiled = if has_error {
        None
    } else {
        Some(generator.finish())
    };
    Report {
        compiled,
        diagnostics,
    }
}

#[derive(Clone, Copy)]
struct NodeRef<'a> {
    node: &'a GraphNode,
    spec: Option<&'a NodeSpec>,
}

type Scope = BTreeSet<(u32, String)>;

struct Generator<'a> {
    document: &'a GraphDocument,
    nodes: BTreeMap<u32, NodeRef<'a>>,
    /// Consumer input pin -> the output pin wired to it.
    data_in: BTreeMap<(u32, String), PinRef>,
    /// Execution output pin -> the node it runs next.
    exec_out: BTreeMap<(u32, String), u32>,
    /// Data outputs some wire reads; an action only names its result when it is read.
    read_outputs: BTreeSet<(u32, String)>,
    diagnostics: Vec<Diagnostic>,
    body: Vec<(String, Option<u32>)>,
    indent: usize,
    uses_state: bool,
    reached: BTreeSet<u32>,
    parts: usize,
}

fn error(node: Option<u32>, pin: Option<&str>, code: &'static str, message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        node,
        pin: pin.map(str::to_string),
        code,
        message,
    }
}

fn warning(node: Option<u32>, code: &'static str, message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        node,
        pin: None,
        code,
        message,
    }
}

impl<'a> Generator<'a> {
    fn new(document: &'a GraphDocument, catalog: &'a Catalog) -> Self {
        let nodes = document
            .nodes
            .iter()
            .map(|node| {
                (
                    node.id,
                    NodeRef {
                        node,
                        spec: catalog.get(&node.kind),
                    },
                )
            })
            .collect();
        Self {
            document,
            nodes,
            data_in: BTreeMap::new(),
            exec_out: BTreeMap::new(),
            read_outputs: BTreeSet::new(),
            diagnostics: Vec::new(),
            body: Vec::new(),
            indent: 0,
            uses_state: false,
            reached: BTreeSet::new(),
            parts: 0,
        }
    }

    fn title(&self, id: u32) -> String {
        match self.nodes.get(&id) {
            Some(NodeRef {
                spec: Some(spec), ..
            }) => format!("nó {id} ({})", spec.title),
            _ => format!("nó {id}"),
        }
    }

    fn run(&mut self) {
        self.check_nodes();
        self.check_edges();
        let mut events: Vec<(String, u32)> = Vec::new();
        for (id, entry) in &self.nodes {
            if let Some(spec) = entry.spec
                && let Lowering::Event { handler } = &spec.lowering
            {
                events.push((handler.clone(), *id));
            }
        }
        events.sort();
        let mut seen: BTreeMap<String, u32> = BTreeMap::new();
        for (handler, id) in events {
            if let Some(first) = seen.get(&handler) {
                let message = format!(
                    "o {} repete o evento do {}; cada evento só pode aparecer uma vez no grafo",
                    self.title(id),
                    self.title(*first)
                );
                self.diagnostics
                    .push(error(Some(id), None, "EVENT_DUPLICATE", message));
                continue;
            }
            seen.insert(handler.clone(), id);
            self.emit_event(id, &handler);
        }
        self.warn_loose_nodes();
    }

    /// Ghost nodes and parameters that do not fit their node.
    fn check_nodes(&mut self) {
        let entries: Vec<NodeRef<'a>> = self.nodes.values().copied().collect();
        for entry in entries {
            let id = entry.node.id;
            let Some(spec) = entry.spec else {
                let message = format!(
                    "o nó {id} usa '{}', que não existe mais; apague o nó ou troque por outro",
                    entry.node.kind
                );
                self.diagnostics
                    .push(error(Some(id), None, "NODE_GHOST", message));
                continue;
            };
            for (name, value) in &entry.node.params {
                if let Some(setting) = spec.setting(name) {
                    match &setting.choices {
                        SettingChoices::OneOf(options) => {
                            let valid = value.as_str().is_some_and(|text| {
                                options.iter().any(|(option, _)| option == text)
                            });
                            if !valid {
                                let message = format!(
                                    "o {}: '{}' não é uma opção de '{}'",
                                    self.title(id),
                                    value,
                                    setting.label
                                );
                                self.diagnostics.push(error(
                                    Some(id),
                                    Some(name),
                                    "SETTING_INVALID",
                                    message,
                                ));
                            }
                        }
                        SettingChoices::Text => {
                            let valid = value.as_str().is_some_and(|text| {
                                !text.trim().is_empty() && text.len() <= MAX_NAME_BYTES
                            });
                            if !valid {
                                let message = format!(
                                    "o {}: '{}' precisa de um texto de 1 a {MAX_NAME_BYTES} letras",
                                    self.title(id),
                                    setting.label
                                );
                                self.diagnostics.push(error(
                                    Some(id),
                                    Some(name),
                                    "SETTING_INVALID",
                                    message,
                                ));
                            }
                        }
                    }
                } else if let Some(pin) = spec.pin(name, Direction::In) {
                    if let PinKind::Data(data_type) = pin.kind
                        && let Err(reason) = literal(value, data_type)
                    {
                        let message = format!("o {}, '{}': {reason}", self.title(id), pin.label);
                        self.diagnostics.push(error(
                            Some(id),
                            Some(name),
                            "VALUE_INVALID",
                            message,
                        ));
                    }
                } else {
                    let message = format!(
                        "o {} tem o parâmetro '{name}', que não existe mais; ele será ignorado",
                        self.title(id)
                    );
                    self.diagnostics
                        .push(warning(Some(id), "PARAM_UNKNOWN", message));
                }
            }
            for setting in &spec.settings {
                if matches!(setting.choices, SettingChoices::Text)
                    && !entry.node.params.contains_key(&setting.name)
                {
                    let message =
                        format!("o {} precisa do campo '{}'", self.title(id), setting.label);
                    self.diagnostics.push(error(
                        Some(id),
                        Some(&setting.name),
                        "SETTING_MISSING",
                        message,
                    ));
                }
            }
        }
    }

    fn check_edges(&mut self) {
        for edge in &self.document.edges {
            let (Some(from), Some(to)) = (
                self.nodes.get(&edge.from.node),
                self.nodes.get(&edge.to.node),
            ) else {
                continue;
            };
            // A ghost node is reported once, on the node; its wires cannot be checked.
            let (Some(from_spec), Some(to_spec)) = (from.spec, to.spec) else {
                continue;
            };
            let Some(from_pin) = from_spec.pin(&edge.from.pin, Direction::Out) else {
                let message = format!(
                    "o nó {} ({}) não tem a saída '{}'",
                    edge.from.node, from_spec.title, edge.from.pin
                );
                self.diagnostics.push(error(
                    Some(edge.from.node),
                    Some(&edge.from.pin),
                    "PIN_UNKNOWN",
                    message,
                ));
                continue;
            };
            let Some(to_pin) = to_spec.pin(&edge.to.pin, Direction::In) else {
                let message = format!(
                    "o nó {} ({}) não tem a entrada '{}'",
                    edge.to.node, to_spec.title, edge.to.pin
                );
                self.diagnostics.push(error(
                    Some(edge.to.node),
                    Some(&edge.to.pin),
                    "PIN_UNKNOWN",
                    message,
                ));
                continue;
            };
            match (from_pin.kind, to_pin.kind) {
                (PinKind::Exec, PinKind::Exec) => {
                    let key = (edge.from.node, edge.from.pin.clone());
                    if self.exec_out.contains_key(&key) {
                        let message = format!(
                            "a saída '{}' do nó {} ({}) já leva a outro nó; para rodar duas coisas, ponha uma depois da outra",
                            from_pin.label, edge.from.node, from_spec.title
                        );
                        self.diagnostics.push(error(
                            Some(edge.from.node),
                            Some(&edge.from.pin),
                            "EXEC_FANOUT",
                            message,
                        ));
                        continue;
                    }
                    let already_entered = self.exec_out.values().any(|next| *next == edge.to.node);
                    if already_entered {
                        let message = format!(
                            "o nó {} ({}) já recebe a execução de outro nó; cada nó roda depois de um só",
                            edge.to.node, to_spec.title
                        );
                        self.diagnostics.push(error(
                            Some(edge.to.node),
                            Some(&edge.to.pin),
                            "EXEC_MERGE",
                            message,
                        ));
                        continue;
                    }
                    self.exec_out.insert(key, edge.to.node);
                }
                (PinKind::Data(source), PinKind::Data(target)) => {
                    if !target.accepts(source) {
                        let message = format!(
                            "não dá para ligar '{}' ({}) em '{}' ({}) do nó {} ({})",
                            from_pin.label,
                            source.label(),
                            to_pin.label,
                            target.label(),
                            edge.to.node,
                            to_spec.title
                        );
                        self.diagnostics.push(error(
                            Some(edge.to.node),
                            Some(&edge.to.pin),
                            "TYPE_MISMATCH",
                            message,
                        ));
                        continue;
                    }
                    let key = (edge.to.node, edge.to.pin.clone());
                    if self.data_in.contains_key(&key) {
                        let message = format!(
                            "a entrada '{}' do nó {} ({}) já tem um valor ligado; ligue só um",
                            to_pin.label, edge.to.node, to_spec.title
                        );
                        self.diagnostics.push(error(
                            Some(edge.to.node),
                            Some(&edge.to.pin),
                            "DATA_TWICE",
                            message,
                        ));
                        continue;
                    }
                    self.read_outputs
                        .insert((edge.from.node, edge.from.pin.clone()));
                    self.data_in.insert(key, edge.from.clone());
                }
                _ => {
                    let message = format!(
                        "não dá para ligar a ordem de execução ('{}') com um valor ('{}')",
                        from_pin.label, to_pin.label
                    );
                    self.diagnostics.push(error(
                        Some(edge.to.node),
                        Some(&edge.to.pin),
                        "KIND_MISMATCH",
                        message,
                    ));
                }
            }
        }
    }

    fn line(&mut self, text: &str, node: Option<u32>) {
        self.body
            .push((format!("{}{text}", "    ".repeat(self.indent)), node));
    }

    fn next(&self, id: u32, pin: &str) -> Option<u32> {
        self.exec_out.get(&(id, pin.to_string())).copied()
    }

    fn emit_event(&mut self, id: u32, handler: &str) {
        let Some(NodeRef {
            spec: Some(spec), ..
        }) = self.nodes.get(&id).copied()
        else {
            return;
        };
        let params: Vec<String> = spec
            .pins
            .iter()
            .filter(|pin| pin.direction == Direction::Out && matches!(pin.kind, PinKind::Data(_)))
            .map(|pin| pin.name.clone())
            .collect();
        if !params.iter().any(|name| name == "entity") {
            let message = format!("o {} não traz o objeto (entity)", self.title(id));
            self.diagnostics
                .push(error(Some(id), None, "EVENT_NO_ENTITY", message));
            return;
        }
        let mut scope = Scope::new();
        for name in &params {
            scope.insert((id, name.clone()));
        }
        self.reached.insert(id);
        self.line(&format!("fn {handler}({}) {{", params.join(", ")), Some(id));
        self.indent += 1;
        let before = self.body.len();
        if let Some(first) = self.next(id, "then") {
            self.emit_chain(first, scope, 1);
        }
        if self.body.len() == before {
            self.line("return", Some(id));
        }
        self.indent -= 1;
        self.line("}", Some(id));
        self.line("", None);
    }

    /// Writes a chain of nodes, following the `then` / `done` wire after each one.
    fn emit_chain(&mut self, start: u32, mut scope: Scope, depth: usize) {
        if depth > MAX_DEPTH {
            let message = format!(
                "blocos demais dentro de blocos (mais de {MAX_DEPTH}); separe a lógica em dois grafos"
            );
            self.diagnostics
                .push(error(Some(start), None, "DEPTH_LIMIT", message));
            return;
        }
        let mut current = Some(start);
        while let Some(id) = current {
            if !self.reached.insert(id) {
                let message = format!(
                    "o {} já foi usado antes; a execução não pode voltar",
                    self.title(id)
                );
                self.diagnostics
                    .push(error(Some(id), None, "EXEC_CYCLE", message));
                return;
            }
            let Some(NodeRef {
                node,
                spec: Some(spec),
            }) = self.nodes.get(&id).copied()
            else {
                return;
            };
            match (&spec.lowering, spec.class) {
                (Lowering::Api { module, name }, NodeClass::Action) => {
                    let mut arguments = Vec::new();
                    let mut ok = true;
                    for pin in data_inputs(spec) {
                        self.parts = 0;
                        match self.input(id, &pin, &scope, &mut Vec::new()) {
                            Some(text) => arguments.push(text),
                            None => ok = false,
                        }
                    }
                    if ok {
                        let call = format!("{module}.{name}({})", arguments.join(", "));
                        if spec.pin("result", Direction::Out).is_some()
                            && self.read_outputs.contains(&(id, "result".to_string()))
                        {
                            self.line(&format!("let n{id} = {call}"), Some(id));
                            scope.insert((id, "result".to_string()));
                        } else {
                            self.line(&call, Some(id));
                        }
                    }
                    current = self.next(id, "then");
                }
                (Lowering::VarSet, NodeClass::Action) => {
                    self.parts = 0;
                    if let Some(value) = self.input(id, "value", &scope, &mut Vec::new()) {
                        self.uses_state = true;
                        let name = text_setting(node, "name");
                        self.line(
                            &format!("graph_state_set(entity, {}, {value})", quote(&name)),
                            Some(id),
                        );
                    }
                    current = self.next(id, "then");
                }
                (Lowering::If, NodeClass::Flow) => {
                    self.parts = 0;
                    let condition = self.input(id, "condition", &scope, &mut Vec::new());
                    let then_start = self.next(id, "then");
                    let else_start = self.next(id, "else");
                    if let Some(condition) = condition {
                        match (then_start, else_start) {
                            (None, None) => {
                                let message = format!(
                                    "o {} não tem nada nos caminhos 'Então' e 'Senão'",
                                    self.title(id)
                                );
                                self.diagnostics
                                    .push(warning(Some(id), "FLOW_EMPTY", message));
                            }
                            (Some(then_start), else_start) => {
                                self.line(&format!("if {condition} {{"), Some(id));
                                self.indent += 1;
                                self.emit_chain(then_start, scope.clone(), depth + 1);
                                self.indent -= 1;
                                if let Some(else_start) = else_start {
                                    self.line("} else {", Some(id));
                                    self.indent += 1;
                                    self.emit_chain(else_start, scope.clone(), depth + 1);
                                    self.indent -= 1;
                                }
                                self.line("}", Some(id));
                            }
                            (None, Some(else_start)) => {
                                self.line(&format!("if not ({condition}) {{"), Some(id));
                                self.indent += 1;
                                self.emit_chain(else_start, scope.clone(), depth + 1);
                                self.indent -= 1;
                                self.line("}", Some(id));
                            }
                        }
                    }
                    current = self.next(id, "done");
                }
                (Lowering::Repeat, NodeClass::Flow) => {
                    self.parts = 0;
                    let count = self.input(id, "count", &scope, &mut Vec::new());
                    if let (Some(count), Some(body)) = (count, self.next(id, "body")) {
                        self.line(&format!("repeat {count} as i{id} {{"), Some(id));
                        self.indent += 1;
                        let mut inner = scope.clone();
                        inner.insert((id, "index".to_string()));
                        self.emit_chain(body, inner, depth + 1);
                        self.indent -= 1;
                        self.line("}", Some(id));
                    } else if self.next(id, "body").is_none() {
                        let message =
                            format!("o {} não tem nada em 'A cada volta'", self.title(id));
                        self.diagnostics
                            .push(warning(Some(id), "FLOW_EMPTY", message));
                    }
                    current = self.next(id, "done");
                }
                (Lowering::Each, NodeClass::Flow) => {
                    self.parts = 0;
                    let list = self.input(id, "list", &scope, &mut Vec::new());
                    if let (Some(list), Some(body)) = (list, self.next(id, "body")) {
                        self.line(
                            &format!("each position{id}, item{id} in {list} {{"),
                            Some(id),
                        );
                        self.indent += 1;
                        let mut inner = scope.clone();
                        inner.insert((id, "index".to_string()));
                        inner.insert((id, "item".to_string()));
                        self.emit_chain(body, inner, depth + 1);
                        self.indent -= 1;
                        self.line("}", Some(id));
                    } else if self.next(id, "body").is_none() {
                        let message = format!("o {} não tem nada em 'Para cada'", self.title(id));
                        self.diagnostics
                            .push(warning(Some(id), "FLOW_EMPTY", message));
                    }
                    current = self.next(id, "done");
                }
                _ => {
                    let message = format!(
                        "o {} não pode estar numa cadeia de execução",
                        self.title(id)
                    );
                    self.diagnostics
                        .push(error(Some(id), None, "NODE_NOT_ACTION", message));
                    return;
                }
            }
        }
    }

    /// The expression for an input pin: its wire, else its constant, else its default.
    fn input(
        &mut self,
        consumer: u32,
        pin_name: &str,
        scope: &Scope,
        stack: &mut Vec<u32>,
    ) -> Option<String> {
        let entry = self.nodes.get(&consumer).copied()?;
        let spec = entry.spec?;
        let pin = spec.pin(pin_name, Direction::In)?;
        let PinKind::Data(data_type) = pin.kind else {
            return None;
        };
        if let Some(source) = self.data_in.get(&(consumer, pin_name.to_string())).cloned() {
            return self.output(consumer, &source, scope, stack);
        }
        if let Some(value) = entry.node.params.get(pin_name) {
            // A bad constant was already reported by `check_nodes`.
            return literal(value, data_type).ok();
        }
        if pin.default == PinDefault::ThisObject {
            return Some("entity".to_string());
        }
        let message = format!(
            "o {}: falta ligar ou preencher '{}'",
            self.title(consumer),
            pin.label
        );
        self.diagnostics.push(error(
            Some(consumer),
            Some(pin_name),
            "PIN_MISSING",
            message,
        ));
        None
    }

    /// The expression for an output pin read by `consumer`.
    fn output(
        &mut self,
        consumer: u32,
        source: &PinRef,
        scope: &Scope,
        stack: &mut Vec<u32>,
    ) -> Option<String> {
        let entry = self.nodes.get(&source.node).copied()?;
        let spec = entry.spec?;
        if spec.class == NodeClass::Value {
            if stack.contains(&source.node) {
                let message = format!(
                    "o {} depende dele mesmo; os valores estão ligados em círculo",
                    self.title(source.node)
                );
                self.diagnostics
                    .push(error(Some(source.node), None, "DATA_CYCLE", message));
                return None;
            }
            stack.push(source.node);
            let text = self.value(source.node, entry, scope, stack);
            stack.pop();
            return text;
        }
        if !scope.contains(&(source.node, source.pin.clone())) {
            let label = spec
                .pin(&source.pin, Direction::Out)
                .map_or(source.pin.as_str(), |pin| pin.label.as_str())
                .to_string();
            let message = format!(
                "o {} usa '{}' do {}, que ainda não existe nesse ponto: o valor só fica pronto depois que esse nó roda, no mesmo caminho",
                self.title(consumer),
                label,
                self.title(source.node)
            );
            self.diagnostics
                .push(error(Some(consumer), None, "VALUE_NOT_READY", message));
            return None;
        }
        Some(match (&spec.lowering, source.pin.as_str()) {
            (Lowering::Event { .. }, pin) => pin.to_string(),
            (Lowering::Repeat, _) => format!("i{}", source.node),
            (Lowering::Each, "item") => format!("item{}", source.node),
            (Lowering::Each, _) => format!("position{}", source.node),
            _ => format!("n{}", source.node),
        })
    }

    /// A pure node written inline.
    fn value(
        &mut self,
        id: u32,
        entry: NodeRef<'a>,
        scope: &Scope,
        stack: &mut Vec<u32>,
    ) -> Option<String> {
        let spec = entry.spec?;
        self.parts += 1;
        if self.parts > MAX_EXPRESSION_PARTS {
            let message = format!(
                "o {} faz parte de uma conta grande demais; guarde um resultado intermediário numa variável",
                self.title(id)
            );
            self.diagnostics
                .push(error(Some(id), None, "EXPRESSION_TOO_BIG", message));
            return None;
        }
        let mut operands = Vec::new();
        let mut ok = true;
        for pin in data_inputs(spec) {
            match self.input(id, &pin, scope, stack) {
                Some(text) => operands.push(text),
                None => ok = false,
            }
        }
        if !ok {
            return None;
        }
        let operand = |index: usize| operands.get(index).cloned().unwrap_or_default();
        let setting = |name: &str| setting_value(entry.node, spec, name);
        Some(match &spec.lowering {
            Lowering::Api { module, name } => format!("{module}.{name}({})", operands.join(", ")),
            Lowering::Compare => format!("({} {} {})", operand(0), setting("operator"), operand(1)),
            Lowering::Logic => format!("({} {} {})", operand(0), setting("operator"), operand(1)),
            Lowering::Math => format!("({} {} {})", operand(0), setting("operator"), operand(1)),
            Lowering::Not => format!("(not {})", operand(0)),
            Lowering::Join => format!("(String({}) + String({}))", operand(0), operand(1)),
            Lowering::Vector => format!(
                "{{\"x\": {}, \"y\": {}, \"z\": {}}}",
                operand(0),
                operand(1),
                operand(2)
            ),
            Lowering::VectorPart => format!("({})[\"{}\"]", operand(0), setting("axis")),
            Lowering::VarGet => {
                self.uses_state = true;
                format!(
                    "graph_state_get(entity, {}, {})",
                    quote(&text_setting(entry.node, "name")),
                    operand(0)
                )
            }
            _ => {
                let message = format!("o {} não é um valor", self.title(id));
                self.diagnostics
                    .push(error(Some(id), None, "NODE_NOT_VALUE", message));
                return None;
            }
        })
    }

    fn warn_loose_nodes(&mut self) {
        let loose: Vec<(u32, bool)> = self
            .nodes
            .iter()
            .filter_map(|(id, entry)| {
                let spec = entry.spec?;
                match spec.class {
                    NodeClass::Action | NodeClass::Flow if !self.reached.contains(id) => {
                        Some((*id, true))
                    }
                    NodeClass::Value => {
                        let read = self.document.edges.iter().any(|edge| edge.from.node == *id);
                        if read { None } else { Some((*id, false)) }
                    }
                    _ => None,
                }
            })
            .collect();
        for (id, chain) in loose {
            let message = if chain {
                format!(
                    "o {} não está ligado a nenhum evento; ele nunca vai rodar",
                    self.title(id)
                )
            } else {
                format!("o {} não é usado por nada", self.title(id))
            };
            let code = if chain { "NODE_LOOSE" } else { "NODE_UNUSED" };
            self.diagnostics.push(warning(Some(id), code, message));
        }
    }

    fn finish(self) -> Compiled {
        let mut lines: Vec<(String, Option<u32>)> = vec![(
            "# Gerado pelo grafo de nós do Petunia. Edite o grafo: este texto é refeito a cada carga."
                .to_string(),
            None,
        )];
        if self.uses_state {
            for text in STATE_PRELUDE.lines() {
                lines.push((text.to_string(), None));
            }
        }
        lines.push((String::new(), None));
        lines.extend(self.body);
        while lines.last().is_some_and(|(text, _)| text.is_empty()) {
            lines.pop();
        }
        let mut text = String::new();
        let mut line_nodes = Vec::with_capacity(lines.len());
        for (line, node) in lines {
            text.push_str(&line);
            text.push('\n');
            line_nodes.push(node);
        }
        let warnings = self
            .diagnostics
            .into_iter()
            .filter(|finding| finding.severity == Severity::Warning)
            .collect();
        Compiled {
            text,
            line_nodes,
            warnings,
        }
    }
}

/// Per-object variables that live while the game runs. The script engine is shared by every object
/// that uses the script, so each variable is keyed by the object.
const STATE_PRELUDE: &str = "var graph_state = {}
fn graph_state_get(entity, name, fallback) {
    let key = String(entity) + \":\" + name
    if graph_state.has(key) {
        return graph_state[key]
    }
    return fallback
}
fn graph_state_set(entity, name, value) {
    graph_state[String(entity) + \":\" + name] = value
}";

/// Names of the data input pins, in catalog order.
fn data_inputs(spec: &NodeSpec) -> Vec<String> {
    spec.pins
        .iter()
        .filter(|pin| pin.direction == Direction::In && matches!(pin.kind, PinKind::Data(_)))
        .map(|pin| pin.name.clone())
        .collect()
}

/// A choice setting's value, the first option when the file does not say.
fn setting_value(node: &GraphNode, spec: &NodeSpec, name: &str) -> String {
    if let Some(Json::String(text)) = node.params.get(name) {
        return text.clone();
    }
    spec.setting(name)
        .and_then(|setting| match &setting.choices {
            SettingChoices::OneOf(options) => options.first().map(|(value, _)| value.clone()),
            SettingChoices::Text => None,
        })
        .unwrap_or_default()
}

fn text_setting(node: &GraphNode, name: &str) -> String {
    match node.params.get(name) {
        Some(Json::String(text)) => text.clone(),
        _ => String::new(),
    }
}

/// A constant as Aipo source, or the reason it does not fit the pin's type.
pub fn literal(value: &Json, data_type: DataType) -> Result<String, String> {
    match data_type {
        DataType::Int => value
            .as_i64()
            .map(|number| number.to_string())
            .ok_or_else(|| "precisa ser um número inteiro".to_string()),
        DataType::Float => value
            .as_f64()
            .filter(|number| number.is_finite())
            .map(float_text)
            .ok_or_else(|| "precisa ser um número".to_string()),
        DataType::Bool => value
            .as_bool()
            .map(|flag| flag.to_string())
            .ok_or_else(|| "precisa ser sim ou não".to_string()),
        DataType::Text => match value.as_str() {
            Some(text) => checked_quote(text),
            None => Err("precisa ser um texto".to_string()),
        },
        DataType::Any => match value {
            Json::Null => Ok("none".to_string()),
            Json::Bool(flag) => Ok(flag.to_string()),
            Json::Number(number) => match number.as_i64() {
                Some(whole) => Ok(whole.to_string()),
                None => number
                    .as_f64()
                    .filter(|decimal| decimal.is_finite())
                    .map(float_text)
                    .ok_or_else(|| "o número não cabe".to_string()),
            },
            Json::String(text) => checked_quote(text),
            _ => Err("só texto, número, sim/não ou nada cabem aqui".to_string()),
        },
        DataType::Dict | DataType::List => {
            Err("este valor precisa vir de outro nó, não de uma constante".to_string())
        }
    }
}

fn float_text(value: f64) -> String {
    let text = format!("{value:?}");
    if text.contains(['e', 'E']) {
        return format!("{value:.9}");
    }
    if text.contains('.') {
        text
    } else {
        format!("{text}.0")
    }
}

fn checked_quote(text: &str) -> Result<String, String> {
    if text.chars().any(|character| {
        (character < ' ' && !matches!(character, '\n' | '\t')) || character == '\u{7f}'
    }) {
        return Err("o texto tem um caractere de controle".to_string());
    }
    Ok(quote(text))
}

/// A string literal with the characters Aipo needs escaped.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_written_as_aipo_literals() {
        assert_eq!(literal(&Json::from(3), DataType::Int).as_deref(), Ok("3"));
        assert_eq!(
            literal(&Json::from(3), DataType::Float).as_deref(),
            Ok("3.0")
        );
        assert_eq!(
            literal(&Json::from(0.5), DataType::Float).as_deref(),
            Ok("0.5")
        );
        assert_eq!(
            literal(&Json::from(true), DataType::Bool).as_deref(),
            Ok("true")
        );
        assert_eq!(literal(&Json::Null, DataType::Any).as_deref(), Ok("none"));
        assert!(
            literal(&Json::from(0.5), DataType::Int).is_err(),
            "a decimal is not an Int"
        );
        assert!(literal(&Json::from("a"), DataType::Float).is_err());
        assert!(
            literal(&Json::from(1), DataType::List).is_err(),
            "lists come from nodes"
        );
    }

    #[test]
    fn text_is_escaped_and_control_characters_are_refused() {
        let quoted = literal(&Json::from("a\"b\\c\nd"), DataType::Text).expect("quotes");
        assert_eq!(quoted, "\"a\\\"b\\\\c\\nd\"");
        assert!(literal(&Json::from("a\u{0}b"), DataType::Text).is_err());
        assert!(literal(&Json::from("a\u{7f}"), DataType::Text).is_err());
    }

    #[test]
    fn a_whole_number_fits_a_decimal_pin_but_not_the_other_way() {
        assert!(DataType::Float.accepts(DataType::Int));
        assert!(!DataType::Int.accepts(DataType::Float));
        assert!(DataType::Any.accepts(DataType::List));
        assert!(DataType::Text.accepts(DataType::Any));
        assert!(!DataType::Text.accepts(DataType::Bool));
    }
}

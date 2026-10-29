//! The first IR → node graph path (DR-34).
//!
//! This is intentionally narrow: event handlers, helper functions, globals, local variables
//! (through `var.*` in events and `local.*` in functions), linear calls, `if`/`elif` per level,
//! `while`, `loop`, `repeat`, `each`, `break`/`continue` and `return`, and expressions made of
//! parameters, literals, calls and operators. Fields, indices and collection literals are refused
//! with a diagnostic instead of being guessed. The graph it produces is ordinary data; the graph
//! compiler still writes the Aipo.

use std::collections::{BTreeMap, BTreeSet};

use petunia_graph::{Catalog, Direction, GraphDocument, GraphEdge, GraphNode, PinKind, PinRef};
use serde_json::{Value as Json, json};

use crate::compile::{ScriptDiagnostic, ScriptSeverity};
use crate::source_ir::{
    BinaryOperator, Expression, Function, Global, Literal, Program, Span, Statement, UnaryOperator,
};

/// Converts a program in the interchange subset into a graph the current compiler accepts.
pub fn to_graph(program: &Program) -> Result<GraphDocument, Vec<ScriptDiagnostic>> {
    let mut converter = Converter {
        catalog: crate::graph_catalog(),
        document: GraphDocument::new(),
        next_id: 1,
        diagnostics: Vec::new(),
        helpers: BTreeMap::new(),
        globals: BTreeMap::new(),
    };
    converter.program(program);
    if converter.diagnostics.is_empty() {
        Ok(converter.document)
    } else {
        Err(converter.diagnostics)
    }
}

struct Converter<'a> {
    catalog: &'a Catalog,
    document: GraphDocument,
    next_id: u32,
    diagnostics: Vec<ScriptDiagnostic>,
    helpers: BTreeMap<String, Vec<String>>,
    globals: BTreeMap<String, GlobalInfo>,
}

#[derive(Clone)]
enum ValueSource {
    Constant(Json),
    Wire(PinRef),
}

struct IfChain<'a> {
    condition: &'a Expression,
    then_branch: &'a [Statement],
    elif_branches: &'a [(Expression, Vec<Statement>)],
    else_branch: Option<&'a [Statement]>,
}

#[derive(Clone)]
struct Context {
    event: u32,
    handler: String,
    parameters: BTreeSet<String>,
    bindings: BTreeMap<String, ValueSource>,
    variables: BTreeMap<String, Variable>,
    loop_depth: usize,
    is_function: bool,
}

#[derive(Clone, Copy)]
struct GlobalInfo {
    mutable: bool,
}

#[derive(Clone)]
struct Variable {
    fallback: ValueSource,
    mutable: bool,
}

impl Context {
    fn slot(&self, name: &str) -> String {
        format!("{}/{}", self.handler, name)
    }
}

impl Converter<'_> {
    fn program(&mut self, program: &Program) {
        for function in &program.functions {
            if !self.is_event(&function.name)
                && self
                    .helpers
                    .insert(function.name.clone(), function.parameters.clone())
                    .is_some()
            {
                self.error(
                    function.span,
                    &format!("a função `{}` aparece mais de uma vez", function.name),
                );
            }
        }
        let module = self.module_context();
        for global in &program.globals {
            self.global(global, &module);
        }
        for function in &program.functions {
            if self.is_event(&function.name) {
                self.event_function(function);
            } else {
                self.helper_function(function);
            }
        }
    }

    fn is_event(&self, name: &str) -> bool {
        self.catalog.get(&format!("event.{name}")).is_some()
    }

    fn module_context(&self) -> Context {
        Context {
            event: 0,
            handler: "module".to_string(),
            parameters: BTreeSet::new(),
            bindings: BTreeMap::new(),
            variables: BTreeMap::new(),
            loop_depth: 0,
            is_function: false,
        }
    }

    fn global(&mut self, global: &Global, context: &Context) {
        if self.globals.contains_key(&global.name) {
            self.error(
                global.span,
                &format!("a global `{}` aparece mais de uma vez", global.name),
            );
            return;
        }
        let value = self.value(&global.value, context, global.span);
        let node = self.add_node(
            "flow.global".to_string(),
            BTreeMap::from([
                ("name".to_string(), Json::from(global.name.clone())),
                (
                    "mode".to_string(),
                    Json::from(if global.mutable { "var" } else { "let" }),
                ),
            ]),
        );
        if let Some(value) = value {
            self.apply_input(node, "value", value);
        }
        self.globals.insert(
            global.name.clone(),
            GlobalInfo {
                mutable: global.mutable,
            },
        );
    }

    fn helper_function(&mut self, function: &Function) {
        if self.document.nodes.iter().any(|node| {
            node.kind == "flow.function"
                && node.params.get("name") == Some(&Json::from(function.name.clone()))
        }) {
            return;
        }
        let node = self.add_node(
            "flow.function".to_string(),
            BTreeMap::from([
                ("name".to_string(), Json::from(function.name.clone())),
                (
                    "params".to_string(),
                    Json::from(function.parameters.join(", ")),
                ),
            ]),
        );
        let mut context = Context {
            event: node,
            handler: function.name.clone(),
            parameters: function.parameters.iter().cloned().collect(),
            bindings: BTreeMap::new(),
            variables: BTreeMap::new(),
            loop_depth: 0,
            is_function: true,
        };
        self.chain(
            &function.body,
            Some(PinRef {
                node,
                pin: "then".to_string(),
            }),
            &mut context,
            function.span,
        );
    }

    fn event_function(&mut self, function: &Function) {
        let kind = format!("event.{}", function.name);
        let Some(spec) = self.catalog.get(&kind) else {
            self.error(
                function.span,
                &format!(
                    "`{}` não é um evento de grafo; o primeiro recorte aceita só on_start, on_update, on_event e on_collision",
                    function.name
                ),
            );
            return;
        };
        let expected: Vec<String> = spec
            .pins
            .iter()
            .filter(|pin| pin.direction == Direction::Out && matches!(pin.kind, PinKind::Data(_)))
            .map(|pin| pin.name.clone())
            .collect();
        if expected != function.parameters {
            self.error(
                function.span,
                &format!(
                    "os parâmetros de `{}` precisam ser ({})",
                    function.name,
                    expected.join(", ")
                ),
            );
            return;
        }
        let event = self.add_node(kind, BTreeMap::new());
        let mut context = Context {
            event,
            handler: function.name.clone(),
            parameters: function.parameters.iter().cloned().collect(),
            bindings: BTreeMap::new(),
            variables: BTreeMap::new(),
            loop_depth: 0,
            is_function: false,
        };
        self.chain(
            &function.body,
            Some(PinRef {
                node: event,
                pin: "then".to_string(),
            }),
            &mut context,
            function.span,
        );
    }

    fn chain(
        &mut self,
        statements: &[Statement],
        mut previous: Option<PinRef>,
        context: &mut Context,
        function: Span,
    ) -> Option<PinRef> {
        for statement in statements {
            let Some(execution) = previous else {
                break;
            };
            previous = match statement {
                Statement::Expression(Expression::Call {
                    function: callee,
                    arguments,
                }) => self.action_call(callee, arguments, execution, context, function),
                Statement::If {
                    condition,
                    then_branch,
                    elif_branches,
                    else_branch,
                } => self.if_statement(
                    IfChain {
                        condition,
                        then_branch,
                        elif_branches,
                        else_branch: else_branch.as_deref(),
                    },
                    execution,
                    context,
                    function,
                ),
                Statement::Return(value) => {
                    let node = self.add_node("flow.return".to_string(), BTreeMap::new());
                    self.connect(execution, node, "in");
                    if let Some(value) = value
                        && let Some(value) = self.value(value, context, function)
                    {
                        self.apply_input(node, "value", value);
                    }
                    None
                }
                Statement::Let { name, value } => {
                    self.declaration(name, value, false, execution, context, function)
                }
                Statement::Var { name, value } => {
                    self.declaration(name, value, true, execution, context, function)
                }
                Statement::Assign { target, value } => {
                    self.assignment(target, value, None, execution, context, function)
                }
                Statement::CompoundAssign {
                    operator,
                    target,
                    value,
                } => self.assignment(target, value, Some(*operator), execution, context, function),
                Statement::Repeat { count, index, body } => self.repeat_statement(
                    count,
                    index.as_deref(),
                    body,
                    execution,
                    context,
                    function,
                ),
                Statement::While { condition, body } => {
                    self.while_statement(condition, body, execution, context, function)
                }
                Statement::Loop { body } => self.loop_statement(body, execution, context, function),
                Statement::Each {
                    names,
                    iterable,
                    body,
                } => self.each_statement(names, iterable, body, execution, context, function),
                Statement::Break => {
                    if context.loop_depth == 0 {
                        self.error(function, "`break` precisa estar dentro de uma repetição");
                        None
                    } else {
                        let node = self.add_node("flow.break".to_string(), BTreeMap::new());
                        self.connect(execution, node, "in");
                        None
                    }
                }
                Statement::Continue => {
                    if context.loop_depth == 0 {
                        self.error(function, "`continue` precisa estar dentro de uma repetição");
                        None
                    } else {
                        let node = self.add_node("flow.continue".to_string(), BTreeMap::new());
                        self.connect(execution, node, "in");
                        None
                    }
                }
                Statement::Expression(_) => {
                    self.error(
                        function,
                        "esta expressão sozinha não é uma chamada de API que o grafo execute",
                    );
                    None
                }
            };
        }
        previous
    }

    fn if_statement(
        &mut self,
        chain: IfChain<'_>,
        execution: PinRef,
        context: &mut Context,
        function: Span,
    ) -> Option<PinRef> {
        let condition = self.value(chain.condition, context, function)?;
        let if_node = self.add_node("flow.if".to_string(), BTreeMap::new());
        self.connect(execution, if_node, "in");
        self.apply_input(if_node, "condition", condition);
        let mut then_context = context.clone();
        self.chain(
            chain.then_branch,
            Some(PinRef {
                node: if_node,
                pin: "then".to_string(),
            }),
            &mut then_context,
            function,
        );
        if let Some(((elif_condition, elif_body), rest)) = chain.elif_branches.split_first() {
            let mut elif_context = context.clone();
            self.if_statement(
                IfChain {
                    condition: elif_condition,
                    then_branch: elif_body,
                    elif_branches: rest,
                    else_branch: chain.else_branch,
                },
                PinRef {
                    node: if_node,
                    pin: "else".to_string(),
                },
                &mut elif_context,
                function,
            );
        } else if let Some(else_branch) = chain.else_branch {
            let mut else_context = context.clone();
            self.chain(
                else_branch,
                Some(PinRef {
                    node: if_node,
                    pin: "else".to_string(),
                }),
                &mut else_context,
                function,
            );
        }
        Some(PinRef {
            node: if_node,
            pin: "done".to_string(),
        })
    }

    fn repeat_statement(
        &mut self,
        count: &Expression,
        index: Option<&str>,
        body: &[Statement],
        execution: PinRef,
        context: &mut Context,
        function: Span,
    ) -> Option<PinRef> {
        let count = self.value(count, context, function)?;
        let repeat = self.add_node("flow.repeat".to_string(), BTreeMap::new());
        self.connect(execution, repeat, "in");
        self.apply_input(repeat, "count", count);
        let mut body_context = context.clone();
        if let Some(index) = index {
            if context.parameters.contains(index)
                || context.variables.contains_key(index)
                || context.bindings.contains_key(index)
            {
                self.error(
                    function,
                    &format!("`{index}` já existe e não pode ser o índice do `repeat`"),
                );
                return None;
            }
            body_context.bindings.insert(
                index.to_string(),
                ValueSource::Wire(PinRef {
                    node: repeat,
                    pin: "index".to_string(),
                }),
            );
        }
        body_context.loop_depth += 1;
        self.chain(
            body,
            Some(PinRef {
                node: repeat,
                pin: "body".to_string(),
            }),
            &mut body_context,
            function,
        );
        Some(PinRef {
            node: repeat,
            pin: "done".to_string(),
        })
    }

    fn while_statement(
        &mut self,
        condition: &Expression,
        body: &[Statement],
        execution: PinRef,
        context: &mut Context,
        function: Span,
    ) -> Option<PinRef> {
        let condition = self.value(condition, context, function)?;
        let node = self.add_node("flow.while".to_string(), BTreeMap::new());
        self.connect(execution, node, "in");
        self.apply_input(node, "condition", condition);
        let mut body_context = context.clone();
        body_context.loop_depth += 1;
        self.chain(
            body,
            Some(PinRef {
                node,
                pin: "body".to_string(),
            }),
            &mut body_context,
            function,
        );
        Some(PinRef {
            node,
            pin: "done".to_string(),
        })
    }

    fn loop_statement(
        &mut self,
        body: &[Statement],
        execution: PinRef,
        context: &mut Context,
        function: Span,
    ) -> Option<PinRef> {
        let node = self.add_node("flow.loop".to_string(), BTreeMap::new());
        self.connect(execution, node, "in");
        let mut body_context = context.clone();
        body_context.loop_depth += 1;
        self.chain(
            body,
            Some(PinRef {
                node,
                pin: "body".to_string(),
            }),
            &mut body_context,
            function,
        );
        Some(PinRef {
            node,
            pin: "done".to_string(),
        })
    }

    fn each_statement(
        &mut self,
        names: &[String],
        iterable: &Expression,
        body: &[Statement],
        execution: PinRef,
        context: &mut Context,
        function: Span,
    ) -> Option<PinRef> {
        let (index_name, item_name) = match names {
            [item] => (None, item),
            [index, item] if index != item => (Some(index), item),
            [_, _] => {
                self.error(
                    function,
                    "as duas variáveis do `each` precisam ter nomes diferentes",
                );
                return None;
            }
            _ => {
                self.error(
                    function,
                    "`each` no grafo aceita uma variável (item/chave) ou duas (posição/chave e item/valor)",
                );
                return None;
            }
        };
        for name in names {
            if context.parameters.contains(name)
                || context.variables.contains_key(name)
                || context.bindings.contains_key(name)
            {
                self.error(
                    function,
                    &format!("`{name}` já existe e não pode ser a variável do `each`"),
                );
                return None;
            }
        }
        let iterable = self.value(iterable, context, function)?;
        let node = self.add_node(
            "flow.each".to_string(),
            BTreeMap::from([(
                "with_index".to_string(),
                Json::from(if index_name.is_some() {
                    "true"
                } else {
                    "false"
                }),
            )]),
        );
        self.connect(execution, node, "in");
        self.apply_input(node, "list", iterable);
        let mut body_context = context.clone();
        if let Some(index_name) = index_name {
            body_context.bindings.insert(
                index_name.clone(),
                ValueSource::Wire(PinRef {
                    node,
                    pin: "index".to_string(),
                }),
            );
        }
        body_context.bindings.insert(
            item_name.clone(),
            ValueSource::Wire(PinRef {
                node,
                pin: "item".to_string(),
            }),
        );
        body_context.loop_depth += 1;
        self.chain(
            body,
            Some(PinRef {
                node,
                pin: "body".to_string(),
            }),
            &mut body_context,
            function,
        );
        Some(PinRef {
            node,
            pin: "done".to_string(),
        })
    }

    fn declaration(
        &mut self,
        name: &str,
        value: &Expression,
        mutable: bool,
        execution: PinRef,
        context: &mut Context,
        function: Span,
    ) -> Option<PinRef> {
        if context.parameters.contains(name) {
            self.error(
                function,
                &format!("`{name}` é um parâmetro do evento e não pode ser redeclarado"),
            );
            return None;
        }
        if context.variables.contains_key(name) || context.bindings.contains_key(name) {
            self.error(
                function,
                &format!(
                    "`{name}` já foi declarada neste handler; sombreamento ainda não vira grafo"
                ),
            );
            return None;
        }
        let value = self.value(value, context, function)?;
        let node = if context.is_function {
            self.add_node(
                "local.declare".to_string(),
                BTreeMap::from([
                    ("name".to_string(), Json::from(name)),
                    (
                        "mode".to_string(),
                        Json::from(if mutable { "var" } else { "let" }),
                    ),
                ]),
            )
        } else {
            let slot = context.slot(name);
            self.add_node(
                "var.set".to_string(),
                BTreeMap::from([("name".to_string(), Json::from(slot))]),
            )
        };
        self.connect(execution, node, "in");
        self.apply_input(node, "value", value.clone());
        context.variables.insert(
            name.to_string(),
            Variable {
                fallback: value,
                mutable,
            },
        );
        Some(PinRef {
            node,
            pin: "then".to_string(),
        })
    }

    fn assignment(
        &mut self,
        target: &Expression,
        value: &Expression,
        compound: Option<BinaryOperator>,
        execution: PinRef,
        context: &mut Context,
        function: Span,
    ) -> Option<PinRef> {
        let Expression::Name(name) = target else {
            self.error(
                function,
                "atribuição só é suportada quando o alvo é uma variável simples",
            );
            return None;
        };
        if context.bindings.contains_key(name) {
            self.error(
                function,
                &format!("`{name}` é o índice do laço e não pode ser atribuída"),
            );
            return None;
        }
        let (current, write_kind, settings) =
            if let Some(variable) = context.variables.get(name).cloned() {
                if !variable.mutable {
                    self.error(
                        function,
                        &format!("`{name}` foi declarada com `let` e não pode ser atribuída"),
                    );
                    return None;
                }
                let current = if context.is_function {
                    self.value_node_with_output(
                        "local.get".to_string(),
                        BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                        Vec::new(),
                        "value",
                    )
                } else {
                    self.value_node_with_output(
                        "var.get".to_string(),
                        BTreeMap::from([("name".to_string(), Json::from(context.slot(name)))]),
                        vec![("fallback".to_string(), variable.fallback)],
                        "value",
                    )
                };
                if context.is_function {
                    (
                        current,
                        "local.set",
                        BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                    )
                } else {
                    (
                        current,
                        "var.set",
                        BTreeMap::from([("name".to_string(), Json::from(context.slot(name)))]),
                    )
                }
            } else if let Some(global) = self.globals.get(name).copied() {
                if !global.mutable {
                    self.error(
                        function,
                        &format!("`{name}` foi declarada com `let` e não pode ser atribuída"),
                    );
                    return None;
                }
                (
                    self.value_node_with_output(
                        "global.get".to_string(),
                        BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                        Vec::new(),
                        "value",
                    ),
                    "global.set",
                    BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                )
            } else {
                self.error(
                    function,
                    &format!("`{name}` não foi declarada neste handler antes da atribuição"),
                );
                return None;
            };
        let right = self.value(value, context, function)?;
        let value = match compound {
            Some(operator) => self.binary_sources(operator, current, right, function)?,
            None => right,
        };
        let node = self.add_node(write_kind.to_string(), settings);
        self.connect(execution, node, "in");
        self.apply_input(node, "value", value);
        Some(PinRef {
            node,
            pin: "then".to_string(),
        })
    }

    fn action_call(
        &mut self,
        callee: &Expression,
        arguments: &[Expression],
        execution: PinRef,
        context: &Context,
        function: Span,
    ) -> Option<PinRef> {
        if let Expression::Name(name) = callee
            && name == "graph_state_set"
        {
            if arguments.len() != 3 {
                self.error(function, "`graph_state_set` precisa de 3 argumentos");
                return None;
            }
            let Some(Expression::Literal(Literal::Text(slot))) = arguments.get(1) else {
                self.error(
                    function,
                    "`graph_state_set` precisa de um nome de slot de texto",
                );
                return None;
            };
            let value = self.value(arguments.get(2)?, context, function)?;
            let node = self.add_node(
                "var.set".to_string(),
                BTreeMap::from([("name".to_string(), Json::from(slot.clone()))]),
            );
            self.connect(execution, node, "in");
            self.apply_input(node, "value", value);
            return Some(PinRef {
                node,
                pin: "then".to_string(),
            });
        }
        if let Expression::Name(name) = callee {
            let Some(parameters) = self.helpers.get(name).cloned() else {
                self.error(
                    function,
                    &format!("`{name}` não é uma função auxiliar deste programa"),
                );
                return None;
            };
            let sources = self.call_arguments(arguments, &parameters, context, function)?;
            let node = self.add_node(
                "flow.call".to_string(),
                BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
            );
            self.connect(execution, node, "in");
            self.apply_inputs(node, sources);
            return Some(PinRef {
                node,
                pin: "then".to_string(),
            });
        }
        let (kind, module, name) = self.api_kind(callee, function)?;
        let Some(spec) = self.catalog.get(&kind) else {
            self.error(
                function,
                &format!("`{module}.{name}` não está no catálogo de nós"),
            );
            return None;
        };
        if !spec.has_exec_in() {
            self.error(
                function,
                &format!(
                    "`{module}.{name}` devolve valor; use a chamada dentro de uma condição ou conta"
                ),
            );
            return None;
        }
        let pins = data_inputs(spec);
        let sources = self.arguments(arguments, &pins, context, function)?;
        let node = self.add_node(kind, BTreeMap::new());
        self.connect(execution, node, "in");
        self.apply_inputs(node, sources);
        Some(PinRef {
            node,
            pin: "then".to_string(),
        })
    }

    fn value(
        &mut self,
        expression: &Expression,
        context: &Context,
        function: Span,
    ) -> Option<ValueSource> {
        match expression {
            Expression::Literal(literal) => Some(ValueSource::Constant(literal_json(literal))),
            Expression::Name(name) => {
                if context.parameters.contains(name) {
                    if context.is_function {
                        Some(self.value_node_with_output(
                            "flow.param".to_string(),
                            BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                            Vec::new(),
                            "value",
                        ))
                    } else {
                        Some(ValueSource::Wire(PinRef {
                            node: context.event,
                            pin: name.clone(),
                        }))
                    }
                } else if let Some(binding) = context.bindings.get(name) {
                    Some(binding.clone())
                } else if let Some(variable) = context.variables.get(name) {
                    if context.is_function {
                        Some(self.value_node_with_output(
                            "local.get".to_string(),
                            BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                            Vec::new(),
                            "value",
                        ))
                    } else {
                        Some(self.value_node_with_output(
                            "var.get".to_string(),
                            BTreeMap::from([("name".to_string(), Json::from(context.slot(name)))]),
                            vec![("fallback".to_string(), variable.fallback.clone())],
                            "value",
                        ))
                    }
                } else if self.globals.contains_key(name) {
                    Some(self.value_node_with_output(
                        "global.get".to_string(),
                        BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                        Vec::new(),
                        "value",
                    ))
                } else {
                    self.error(
                        function,
                        &format!("`{name}` não é um parâmetro nem uma variável declarada"),
                    );
                    None
                }
            }
            Expression::Binary {
                operator,
                left,
                right,
            } => self.binary_value(*operator, left, right, context, function),
            Expression::Unary { operator, operand } => match operator {
                UnaryOperator::Positive => self.value(operand, context, function),
                UnaryOperator::Not => {
                    let operand = self.value(operand, context, function)?;
                    Some(self.value_node(
                        "op.not".to_string(),
                        BTreeMap::new(),
                        vec![("a".to_string(), operand)],
                    ))
                }
                UnaryOperator::Negate => {
                    let operand = self.value(operand, context, function)?;
                    Some(self.value_node(
                        "op.math".to_string(),
                        BTreeMap::from([("operator".to_string(), Json::from("-"))]),
                        vec![
                            ("a".to_string(), ValueSource::Constant(json!(0.0))),
                            ("b".to_string(), operand),
                        ],
                    ))
                }
            },
            Expression::Call {
                function: callee,
                arguments,
            } => {
                if let Expression::Name(name) = callee.as_ref()
                    && name == "graph_state_get"
                {
                    if arguments.len() != 3 {
                        self.error(function, "`graph_state_get` precisa de 3 argumentos");
                        return None;
                    }
                    let Some(Expression::Literal(Literal::Text(slot))) = arguments.get(1) else {
                        self.error(
                            function,
                            "`graph_state_get` precisa de um nome de slot de texto",
                        );
                        return None;
                    };
                    let fallback = self.value(arguments.get(2)?, context, function)?;
                    return Some(self.value_node_with_output(
                        "var.get".to_string(),
                        BTreeMap::from([("name".to_string(), Json::from(slot.clone()))]),
                        vec![("fallback".to_string(), fallback)],
                        "value",
                    ));
                }
                if let Expression::Name(name) = callee.as_ref() {
                    let Some(parameters) = self.helpers.get(name).cloned() else {
                        self.error(
                            function,
                            &format!("`{name}` não é uma função auxiliar deste programa"),
                        );
                        return None;
                    };
                    let sources = self.call_arguments(arguments, &parameters, context, function)?;
                    return Some(self.value_node(
                        "flow.call_value".to_string(),
                        BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                        sources,
                    ));
                }
                let (kind, module, name) = self.api_kind(callee, function)?;
                let Some(spec) = self.catalog.get(&kind) else {
                    self.error(
                        function,
                        &format!("`{module}.{name}` não está no catálogo de nós"),
                    );
                    return None;
                };
                if spec.has_exec_in() {
                    self.error(
                        function,
                        &format!("`{module}.{name}` é uma ação e não pode ser usada como valor"),
                    );
                    return None;
                }
                let pins = data_inputs(spec);
                let sources = self.arguments(arguments, &pins, context, function)?;
                Some(self.value_node(kind, BTreeMap::new(), sources))
            }
            Expression::Field { target, name } => {
                let target = self.value(target, context, function)?;
                Some(self.value_node(
                    "op.field".to_string(),
                    BTreeMap::from([("name".to_string(), Json::from(name.clone()))]),
                    vec![("target".to_string(), target)],
                ))
            }
            Expression::Index { target, index } => {
                let target = self.value(target, context, function)?;
                let index = self.value(index, context, function)?;
                Some(self.value_node(
                    "op.index".to_string(),
                    BTreeMap::new(),
                    vec![("target".to_string(), target), ("index".to_string(), index)],
                ))
            }
            Expression::List(values) => {
                if values.len() > 16 {
                    self.error(function, "lista com mais de 16 itens ainda não vira grafo");
                    return None;
                }
                let mut inputs = Vec::new();
                for (index, value) in values.iter().enumerate() {
                    inputs.push((
                        format!("item{}", index + 1),
                        self.value(value, context, function)?,
                    ));
                }
                Some(self.value_node("op.list".to_string(), BTreeMap::new(), inputs))
            }
            Expression::Dict(entries) => {
                if entries.len() > 8 {
                    self.error(
                        function,
                        "dicionário com mais de 8 pares ainda não vira grafo",
                    );
                    return None;
                }
                let mut inputs = Vec::new();
                for (index, (key, value)) in entries.iter().enumerate() {
                    inputs.push((
                        format!("key{}", index + 1),
                        self.value(key, context, function)?,
                    ));
                    inputs.push((
                        format!("value{}", index + 1),
                        self.value(value, context, function)?,
                    ));
                }
                Some(self.value_node("op.dict".to_string(), BTreeMap::new(), inputs))
            }
        }
    }

    fn binary_value(
        &mut self,
        operator: BinaryOperator,
        left: &Expression,
        right: &Expression,
        context: &Context,
        function: Span,
    ) -> Option<ValueSource> {
        let left = self.value(left, context, function)?;
        let right = self.value(right, context, function)?;
        self.binary_sources(operator, left, right, function)
    }

    fn binary_sources(
        &mut self,
        operator: BinaryOperator,
        left: ValueSource,
        right: ValueSource,
        function: Span,
    ) -> Option<ValueSource> {
        let (kind, symbol) = match operator {
            BinaryOperator::Add => ("op.math", "+"),
            BinaryOperator::Subtract => ("op.math", "-"),
            BinaryOperator::Multiply => ("op.math", "*"),
            BinaryOperator::Divide => ("op.math", "/"),
            BinaryOperator::Remainder => ("op.math", "%"),
            BinaryOperator::Equal => ("op.compare", "=="),
            BinaryOperator::NotEqual => ("op.compare", "!="),
            BinaryOperator::Less => ("op.compare", "<"),
            BinaryOperator::LessEqual => ("op.compare", "<="),
            BinaryOperator::Greater => ("op.compare", ">"),
            BinaryOperator::GreaterEqual => ("op.compare", ">="),
            BinaryOperator::And => ("op.logic", "and"),
            BinaryOperator::Or => ("op.logic", "or"),
            BinaryOperator::IntegerDivide => {
                self.error(function, "divisão inteira ainda não tem nó");
                return None;
            }
        };
        let settings = BTreeMap::from([("operator".to_string(), Json::from(symbol))]);
        Some(self.value_node(
            kind.to_string(),
            settings,
            vec![("a".to_string(), left), ("b".to_string(), right)],
        ))
    }

    fn value_node(
        &mut self,
        kind: String,
        settings: BTreeMap<String, Json>,
        inputs: Vec<(String, ValueSource)>,
    ) -> ValueSource {
        self.value_node_with_output(kind, settings, inputs, "result")
    }

    fn value_node_with_output(
        &mut self,
        kind: String,
        settings: BTreeMap<String, Json>,
        inputs: Vec<(String, ValueSource)>,
        output: &str,
    ) -> ValueSource {
        let node = self.add_node(kind, settings);
        for (pin, source) in inputs {
            self.apply_input(node, &pin, source);
        }
        ValueSource::Wire(PinRef {
            node,
            pin: output.to_string(),
        })
    }

    fn call_arguments(
        &mut self,
        arguments: &[Expression],
        parameters: &[String],
        context: &Context,
        function: Span,
    ) -> Option<Vec<(String, ValueSource)>> {
        if arguments.len() != parameters.len() {
            self.error(
                function,
                &format!(
                    "a chamada tem {} argumento(s), mas a função espera {}",
                    arguments.len(),
                    parameters.len()
                ),
            );
            return None;
        }
        let mut sources = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            sources.push((
                format!("arg{}", index + 1),
                self.value(argument, context, function)?,
            ));
        }
        Some(sources)
    }

    fn arguments(
        &mut self,
        arguments: &[Expression],
        pins: &[String],
        context: &Context,
        function: Span,
    ) -> Option<Vec<(String, ValueSource)>> {
        if arguments.len() != pins.len() {
            self.error(
                function,
                &format!(
                    "a chamada tem {} argumento(s), mas o nó espera {}; argumentos nomeados e variádicos ainda não viram grafo",
                    arguments.len(),
                    pins.len()
                ),
            );
            return None;
        }
        let mut sources = Vec::new();
        for (argument, pin) in arguments.iter().zip(pins) {
            sources.push((pin.clone(), self.value(argument, context, function)?));
        }
        Some(sources)
    }

    fn api_kind(
        &mut self,
        callee: &Expression,
        function: Span,
    ) -> Option<(String, String, String)> {
        let Expression::Field { target, name } = callee else {
            self.error(function, "só chamadas `módulo.função` viram nó de API");
            return None;
        };
        let Expression::Name(module) = target.as_ref() else {
            self.error(function, "o lado esquerdo da chamada precisa ser um módulo");
            return None;
        };
        Some((format!("api.{module}.{name}"), module.clone(), name.clone()))
    }

    fn apply_inputs(&mut self, node: u32, sources: Vec<(String, ValueSource)>) {
        for (pin, source) in sources {
            self.apply_input(node, &pin, source);
        }
    }

    fn apply_input(&mut self, node: u32, pin: &str, source: ValueSource) {
        match source {
            ValueSource::Constant(value) => {
                self.set_param(node, pin, value);
            }
            ValueSource::Wire(source) => {
                self.document.edges.push(GraphEdge {
                    from: source,
                    to: PinRef {
                        node,
                        pin: pin.to_string(),
                    },
                });
            }
        }
    }

    fn set_param(&mut self, node: u32, pin: &str, value: Json) {
        if let Some(found) = self
            .document
            .nodes
            .iter_mut()
            .find(|found| found.id == node)
        {
            found.params.insert(pin.to_string(), value);
        }
    }

    fn add_node(&mut self, kind: String, params: BTreeMap<String, Json>) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.document.nodes.push(GraphNode { id, kind, params });
        id
    }

    fn connect(&mut self, from: PinRef, to: u32, pin: &str) {
        self.document.edges.push(GraphEdge {
            from,
            to: PinRef {
                node: to,
                pin: pin.to_string(),
            },
        });
    }

    fn error(&mut self, span: Span, message: &str) {
        self.diagnostics.push(ScriptDiagnostic {
            severity: ScriptSeverity::Error,
            code: "IR_GRAPH_UNSUPPORTED".to_string(),
            message: message.to_string(),
            line: span.line,
            column: span.column,
            end_line: span.line,
            end_column: span.column,
            notes: vec![
                "o texto continua válido como script; ele só não vira grafo neste recorte"
                    .to_string(),
            ],
        });
    }
}

fn data_inputs(spec: &petunia_graph::NodeSpec) -> Vec<String> {
    spec.pins
        .iter()
        .filter(|pin| pin.direction == Direction::In && matches!(pin.kind, PinKind::Data(_)))
        .map(|pin| pin.name.clone())
        .collect()
}

fn literal_json(literal: &Literal) -> Json {
    match literal {
        Literal::Integer(value) => json!(value),
        Literal::Float(value) => json!(value),
        Literal::Bool(value) => json!(value),
        Literal::Text(value) => json!(value),
        Literal::None => Json::Null,
    }
}

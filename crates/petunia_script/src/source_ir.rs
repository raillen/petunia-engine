//! A small, explicit Aipo ⇄ IR path.
//!
//! This is the first step of the interchange work (DR-34), not a promise that every Aipo program
//! becomes a graph. It reads a deliberately limited subset of Aipo into plain data and writes that
//! data back as canonical Aipo. Anything outside the subset is refused with a line and a reason;
//! nothing is silently approximated.
//!
//! The subset is:
//! - top-level `let`/`var` globals;
//! - plain functions without directives, `async`, type annotations or default arguments;
//! - `let`, `var`, assignment, compound assignment, `if`/`elif`/`else`, `while`, `loop`,
//!   `repeat`, `each`, `break`, `continue`, `return` and expression statements;
//! - literals, names, field access, indexing, unary and binary operators, calls with positional
//!   arguments, lists and dictionaries.
//!
//! Closures, structs, imports, `attempt`, `await`, safe navigation, ranges, pipelines and
//! language-specific sugar are refused for now. The module is public so [`crate::ir_graph`] and
//! the tests can use it without moving the code again.

use aipo_ast::{BinaryOp, Literal as AipoLiteral, StringPrefix, UnaryOp};
use aipo_diagnostics::Severity;
use aipo_hir::{HirExpr, HirFunctionDecl, HirIfStmt, HirItem, HirProgram, HirStmt};
use aipo_source::{Source, SourceId, SourceSpan};

use crate::compile::ScriptDiagnostic;
use crate::compile::convert;

/// A place in the original text, for a diagnostic that outlives the parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

/// A program in the interchange subset.
#[derive(Clone, Debug, PartialEq)]
pub struct Program {
    pub globals: Vec<Global>,
    pub functions: Vec<Function>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Global {
    pub name: String,
    pub mutable: bool,
    pub value: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    Let {
        name: String,
        value: Expression,
    },
    Var {
        name: String,
        value: Expression,
    },
    Assign {
        target: Expression,
        value: Expression,
    },
    CompoundAssign {
        operator: BinaryOperator,
        target: Expression,
        value: Expression,
    },
    If {
        condition: Expression,
        then_branch: Vec<Statement>,
        elif_branches: Vec<(Expression, Vec<Statement>)>,
        else_branch: Option<Vec<Statement>>,
    },
    While {
        condition: Expression,
        body: Vec<Statement>,
    },
    Loop {
        body: Vec<Statement>,
    },
    Repeat {
        count: Expression,
        index: Option<String>,
        body: Vec<Statement>,
    },
    Each {
        names: Vec<String>,
        iterable: Expression,
        body: Vec<Statement>,
    },
    Break,
    Continue,
    Return(Option<Expression>),
    Expression(Expression),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    Literal(Literal),
    Name(String),
    Field {
        target: Box<Expression>,
        name: String,
    },
    Index {
        target: Box<Expression>,
        index: Box<Expression>,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expression>,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Call {
        function: Box<Expression>,
        arguments: Vec<Expression>,
    },
    List(Vec<Expression>),
    Dict(Vec<(Expression, Expression)>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Literal {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOperator {
    Negate,
    Positive,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    IntegerDivide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

/// Reads `text` into the interchange subset, or reports every unsupported construct.
pub fn parse_source(name: &str, text: &str) -> Result<Program, Vec<ScriptDiagnostic>> {
    let source = Source::new(SourceId::next(), name, text);
    let (ast, diagnostics) = aipo_syntax::parse(&source);
    let mut found: Vec<ScriptDiagnostic> = Vec::new();
    for diagnostic in &diagnostics {
        if diagnostic.severity == Severity::Error {
            found.push(convert(&source, diagnostic));
        }
    }
    if !found.is_empty() {
        return Err(found);
    }
    let program = aipo_hir::lower(ast);
    let mut reader = Reader {
        source: &source,
        diagnostics: Vec::new(),
    };
    let result = reader.program(&program);
    if reader.diagnostics.is_empty() {
        result.ok_or_else(|| {
            vec![ScriptDiagnostic::general(
                "IR_EMPTY",
                "o programa não pôde ser lido",
            )]
        })
    } else {
        Err(reader.diagnostics)
    }
}

/// Writes the subset back as canonical Aipo. The output is stable: reading it again gives the same
/// program.
pub fn emit_aipo(program: &Program) -> String {
    let mut writer = Writer::default();
    for global in &program.globals {
        writer.line(&format!(
            "{} {} = {}",
            if global.mutable { "var" } else { "let" },
            global.name,
            writer.expression(&global.value)
        ));
    }
    if !program.globals.is_empty() && !program.functions.is_empty() {
        writer.blank();
    }
    for (index, function) in program.functions.iter().enumerate() {
        if index > 0 {
            writer.blank();
        }
        writer.line(&format!(
            "fn {}({}) {{",
            function.name,
            function.parameters.join(", ")
        ));
        writer.indent += 1;
        writer.statements(&function.body);
        writer.indent -= 1;
        writer.line("}");
    }
    writer.output
}

struct Reader<'a> {
    source: &'a Source,
    diagnostics: Vec<ScriptDiagnostic>,
}

impl Reader<'_> {
    fn program(&mut self, program: &HirProgram) -> Option<Program> {
        let mut globals = Vec::new();
        for statement in &program.statements {
            match statement {
                HirStmt::Let(name, value, span) => {
                    if let Some(value) = self.expression(value) {
                        globals.push(Global {
                            name: name.clone(),
                            mutable: false,
                            value,
                            span: self.span(*span),
                        });
                    } else {
                        self.unsupported(*span, "declaração global fora do subconjunto");
                    }
                }
                HirStmt::Var(name, value, span) => {
                    if let Some(value) = self.expression(value) {
                        globals.push(Global {
                            name: name.clone(),
                            mutable: true,
                            value,
                            span: self.span(*span),
                        });
                    } else {
                        self.unsupported(*span, "declaração global fora do subconjunto");
                    }
                }
                other => {
                    self.unsupported(statement_span(other), "comando global fora do subconjunto");
                }
            }
        }
        let mut functions = Vec::new();
        for item in &program.items {
            match item {
                HirItem::Fn(function) => {
                    if let Some(function) = self.function(function) {
                        functions.push(function);
                    }
                }
                other => {
                    self.unsupported(item_span(other), "declaração fora do subconjunto");
                }
            }
        }
        Some(Program { globals, functions })
    }

    fn function(&mut self, function: &HirFunctionDecl) -> Option<Function> {
        if function.is_async || !function.directives.is_empty() || function.return_type.is_some() {
            self.unsupported(
                function.span,
                "função com `async`, diretiva ou tipo de retorno",
            );
            return None;
        }
        let mut parameters = Vec::new();
        for parameter in &function.params {
            if parameter.is_mut
                || parameter.is_self
                || parameter.type_annotation.is_some()
                || parameter.default.is_some()
            {
                self.unsupported(parameter.span, "parâmetro fora do subconjunto");
            }
            parameters.push(parameter.name.clone());
        }
        let body = self.statements(&function.body);
        Some(Function {
            name: function.name.clone(),
            parameters,
            body,
            span: self.span(function.span),
        })
    }

    fn span(&self, span: SourceSpan) -> Span {
        let location = self
            .source
            .location(span.start)
            .unwrap_or(aipo_source::SourceLocation {
                line: 1,
                column: 1,
                offset: 0,
            });
        Span {
            line: location.line,
            column: location.column,
        }
    }

    fn statements(&mut self, statements: &[HirStmt]) -> Vec<Statement> {
        let mut converted = Vec::new();
        for statement in statements {
            if let Some(statement) = self.statement(statement) {
                converted.push(statement);
            }
        }
        converted
    }

    fn statement(&mut self, statement: &HirStmt) -> Option<Statement> {
        match statement {
            HirStmt::Let(name, value, span) => self
                .expression(value)
                .map(|value| Statement::Let {
                    name: name.clone(),
                    value,
                })
                .or_else(|| {
                    self.unsupported(*span, "declaração fora do subconjunto");
                    None
                }),
            HirStmt::Var(name, value, span) => self
                .expression(value)
                .map(|value| Statement::Var {
                    name: name.clone(),
                    value,
                })
                .or_else(|| {
                    self.unsupported(*span, "declaração fora do subconjunto");
                    None
                }),
            HirStmt::Assign(target, value, span) => {
                let target = self.expression(target);
                let value = self.expression(value);
                match (target, value) {
                    (Some(target), Some(value)) => Some(Statement::Assign { target, value }),
                    _ => {
                        self.unsupported(*span, "atribuição fora do subconjunto");
                        None
                    }
                }
            }
            HirStmt::CompoundAssign(operator, target, value, span) => {
                let Some(operator) = binary_operator(*operator) else {
                    self.unsupported(*span, "operador de atribuição fora do subconjunto");
                    return None;
                };
                let target = self.expression(target);
                let value = self.expression(value);
                match (target, value) {
                    (Some(target), Some(value)) => Some(Statement::CompoundAssign {
                        operator,
                        target,
                        value,
                    }),
                    _ => {
                        self.unsupported(*span, "atribuição composta fora do subconjunto");
                        None
                    }
                }
            }
            HirStmt::If(statement) => self.if_statement(statement),
            HirStmt::While(condition, body, span) => {
                let Some(condition) = self.expression(condition) else {
                    self.unsupported(*span, "condição de `while` fora do subconjunto");
                    return None;
                };
                Some(Statement::While {
                    condition,
                    body: self.statements(body),
                })
            }
            HirStmt::Repeat(count, index, body, span) => {
                let Some(count) = self.expression(count) else {
                    self.unsupported(*span, "contagem de `repeat` fora do subconjunto");
                    return None;
                };
                Some(Statement::Repeat {
                    count,
                    index: index.clone(),
                    body: self.statements(body),
                })
            }
            HirStmt::Loop(body, _) => Some(Statement::Loop {
                body: self.statements(body),
            }),
            HirStmt::Each(names, iterable, body, span) => {
                let Some(iterable) = self.expression(iterable) else {
                    self.unsupported(*span, "coleção de `each` fora do subconjunto");
                    return None;
                };
                Some(Statement::Each {
                    names: names.clone(),
                    iterable,
                    body: self.statements(body),
                })
            }
            HirStmt::Break(_) => Some(Statement::Break),
            HirStmt::Continue(_) => Some(Statement::Continue),
            HirStmt::Return(value, span) => {
                let value = match value {
                    Some(value) => self.expression(value),
                    None => None,
                };
                if value.is_none() && matches!(statement, HirStmt::Return(Some(_), _)) {
                    self.unsupported(*span, "valor de retorno fora do subconjunto");
                    return None;
                }
                Some(Statement::Return(value))
            }
            HirStmt::Expr(expression) => self.expression(expression).map(Statement::Expression),
            other => {
                self.unsupported(statement_span(other), "comando fora do subconjunto");
                None
            }
        }
    }

    fn if_statement(&mut self, statement: &HirIfStmt) -> Option<Statement> {
        let Some(condition) = self.expression(&statement.condition) else {
            self.unsupported(statement.span, "condição de `if` fora do subconjunto");
            return None;
        };
        let then_branch = self.statements(&statement.then_branch);
        let mut elif_branches = Vec::new();
        for (condition, body) in &statement.elif_branches {
            let Some(condition) = self.expression(condition) else {
                self.unsupported(statement.span, "condição de `elif` fora do subconjunto");
                return None;
            };
            elif_branches.push((condition, self.statements(body)));
        }
        let else_branch = statement
            .else_branch
            .as_ref()
            .map(|body| self.statements(body));
        Some(Statement::If {
            condition,
            then_branch,
            elif_branches,
            else_branch,
        })
    }

    fn expression(&mut self, expression: &HirExpr) -> Option<Expression> {
        match expression {
            HirExpr::Literal(literal, span) => self.literal(literal, *span),
            HirExpr::Identifier(name, _) => Some(Expression::Name(name.clone())),
            HirExpr::Unary(operator, operand, span) => {
                let Some(operator) = unary_operator(*operator) else {
                    self.unsupported(*span, "operador unário fora do subconjunto");
                    return None;
                };
                Some(Expression::Unary {
                    operator,
                    operand: Box::new(self.expression(operand)?),
                })
            }
            HirExpr::Binary(operator, left, right, span) => {
                let Some(operator) = binary_operator(*operator) else {
                    self.unsupported(*span, "operador binário fora do subconjunto");
                    return None;
                };
                Some(Expression::Binary {
                    operator,
                    left: Box::new(self.expression(left)?),
                    right: Box::new(self.expression(right)?),
                })
            }
            HirExpr::Call(function, arguments, span) => {
                let function = self.expression(function)?;
                let mut converted = Vec::new();
                for argument in arguments {
                    if argument.name.is_some() {
                        self.unsupported(argument.span, "argumento nomeado fora do subconjunto");
                        return None;
                    }
                    converted.push(self.expression(&argument.value)?);
                }
                let _ = span;
                Some(Expression::Call {
                    function: Box::new(function),
                    arguments: converted,
                })
            }
            HirExpr::Dot(target, name, _) => Some(Expression::Field {
                target: Box::new(self.expression(target)?),
                name: name.clone(),
            }),
            HirExpr::Index(target, index, _) => Some(Expression::Index {
                target: Box::new(self.expression(target)?),
                index: Box::new(self.expression(index)?),
            }),
            HirExpr::List(values, _) => {
                let mut converted = Vec::new();
                for value in values {
                    converted.push(self.expression(value)?);
                }
                Some(Expression::List(converted))
            }
            HirExpr::Dict(entries, _) => {
                let mut converted = Vec::new();
                for (key, value) in entries {
                    converted.push((self.expression(key)?, self.expression(value)?));
                }
                Some(Expression::Dict(converted))
            }
            other => {
                self.unsupported(expression_span(other), "expressão fora do subconjunto");
                None
            }
        }
    }

    fn literal(&mut self, literal: &AipoLiteral, span: SourceSpan) -> Option<Expression> {
        let value = match literal {
            AipoLiteral::Int(text) => match text.parse::<i64>() {
                Ok(value) => Literal::Integer(value),
                Err(_) => {
                    self.unsupported(span, "inteiro fora do intervalo de 64 bits com sinal");
                    return None;
                }
            },
            AipoLiteral::Float(text) => match text.parse::<f64>() {
                Ok(value) if value.is_finite() => Literal::Float(value),
                _ => {
                    self.unsupported(span, "decimal inválido ou não finito");
                    return None;
                }
            },
            AipoLiteral::Bool(value) => Literal::Bool(*value),
            AipoLiteral::String(text, StringPrefix::Normal) => Literal::Text(text.clone()),
            AipoLiteral::String(_, _) => {
                self.unsupported(span, "texto com prefixo fora do subconjunto");
                return None;
            }
            AipoLiteral::None => Literal::None,
        };
        Some(Expression::Literal(value))
    }

    fn unsupported(&mut self, span: SourceSpan, reason: &str) {
        let start = self
            .source
            .location(span.start)
            .unwrap_or(aipo_source::SourceLocation {
                line: 1,
                column: 1,
                offset: 0,
            });
        let end = self.source.location(span.end).unwrap_or(start);
        self.diagnostics.push(ScriptDiagnostic {
            severity: crate::compile::ScriptSeverity::Error,
            code: "IR_UNSUPPORTED".to_string(),
            message: reason.to_string(),
            line: start.line,
            column: start.column,
            end_line: end.line,
            end_column: end.column,
            notes: vec!["o texto continua válido como script; apenas não vira IR/nós".to_string()],
        });
    }
}

fn unary_operator(operator: UnaryOp) -> Option<UnaryOperator> {
    match operator {
        UnaryOp::Neg => Some(UnaryOperator::Negate),
        UnaryOp::Pos => Some(UnaryOperator::Positive),
        UnaryOp::Not => Some(UnaryOperator::Not),
    }
}

fn binary_operator(operator: BinaryOp) -> Option<BinaryOperator> {
    match operator {
        BinaryOp::Add => Some(BinaryOperator::Add),
        BinaryOp::Sub => Some(BinaryOperator::Subtract),
        BinaryOp::Mul => Some(BinaryOperator::Multiply),
        BinaryOp::Div => Some(BinaryOperator::Divide),
        BinaryOp::IntDiv => Some(BinaryOperator::IntegerDivide),
        BinaryOp::Mod => Some(BinaryOperator::Remainder),
        BinaryOp::Equal => Some(BinaryOperator::Equal),
        BinaryOp::NotEqual => Some(BinaryOperator::NotEqual),
        BinaryOp::Less => Some(BinaryOperator::Less),
        BinaryOp::LessEqual => Some(BinaryOperator::LessEqual),
        BinaryOp::Greater => Some(BinaryOperator::Greater),
        BinaryOp::GreaterEqual => Some(BinaryOperator::GreaterEqual),
        BinaryOp::And => Some(BinaryOperator::And),
        BinaryOp::Or => Some(BinaryOperator::Or),
        BinaryOp::Is
        | BinaryOp::IsNullable
        | BinaryOp::OrElse
        | BinaryOp::Pipeline
        | BinaryOp::Range => None,
    }
}

fn statement_span(statement: &HirStmt) -> SourceSpan {
    match statement {
        HirStmt::Let(_, _, span)
        | HirStmt::Var(_, _, span)
        | HirStmt::Assign(_, _, span)
        | HirStmt::CompoundAssign(_, _, _, span)
        | HirStmt::Loop(_, span)
        | HirStmt::While(_, _, span)
        | HirStmt::Repeat(_, _, _, span)
        | HirStmt::Each(_, _, _, span)
        | HirStmt::Break(span)
        | HirStmt::Continue(span)
        | HirStmt::Return(_, span)
        | HirStmt::Fail(_, span)
        | HirStmt::AwaitDo(_, span) => *span,
        HirStmt::If(statement) => statement.span,
        HirStmt::Match(statement) => statement.span,
        HirStmt::Attempt(statement) => statement.span,
        HirStmt::FnDecl(function) => function.span,
        HirStmt::Expr(expression) => expression_span(expression),
    }
}

fn item_span(item: &HirItem) -> SourceSpan {
    match item {
        HirItem::Fn(function) => function.span,
        HirItem::Struct(declaration) => declaration.span,
        HirItem::Enum(declaration) => declaration.span,
        HirItem::Impl(block) => block.span,
        HirItem::Interface(declaration) => declaration.span,
        HirItem::Batch(batch) => batch.span,
        HirItem::Import(import) => import.span,
        HirItem::Export(export) => export.span,
    }
}

fn expression_span(expression: &HirExpr) -> SourceSpan {
    match expression {
        HirExpr::Literal(_, span)
        | HirExpr::Identifier(_, span)
        | HirExpr::Unary(_, _, span)
        | HirExpr::Binary(_, _, _, span)
        | HirExpr::Call(_, _, span)
        | HirExpr::Dot(_, _, span)
        | HirExpr::QuestionDot(_, _, span)
        | HirExpr::Index(_, _, span)
        | HirExpr::List(_, span)
        | HirExpr::Dict(_, span)
        | HirExpr::Construct(_, _, span)
        | HirExpr::If(_, _, _, span)
        | HirExpr::OrElse(_, _, span)
        | HirExpr::Await(_, span)
        | HirExpr::Try(_, span)
        | HirExpr::With(_, _, span) => *span,
        HirExpr::Fn(function) => function.span,
    }
}

#[derive(Default)]
struct Writer {
    output: String,
    indent: usize,
}

impl Writer {
    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn blank(&mut self) {
        if !self.output.ends_with("\n\n") {
            self.output.push('\n');
        }
    }

    fn statements(&mut self, statements: &[Statement]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    fn statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Let { name, value } => {
                self.line(&format!("let {name} = {}", self.expression(value)));
            }
            Statement::Var { name, value } => {
                self.line(&format!("var {name} = {}", self.expression(value)));
            }
            Statement::Assign { target, value } => {
                self.line(&format!(
                    "{} = {}",
                    self.expression(target),
                    self.expression(value)
                ));
            }
            Statement::CompoundAssign {
                operator,
                target,
                value,
            } => {
                self.line(&format!(
                    "{} {}= {}",
                    self.expression(target),
                    operator.symbol(),
                    self.expression(value)
                ));
            }
            Statement::If {
                condition,
                then_branch,
                elif_branches,
                else_branch,
            } => {
                self.line(&format!("if {} {{", self.expression(condition)));
                self.indent += 1;
                self.statements(then_branch);
                self.indent -= 1;
                for (condition, body) in elif_branches {
                    self.line(&format!("}} elif {} {{", self.expression(condition)));
                    self.indent += 1;
                    self.statements(body);
                    self.indent -= 1;
                }
                if let Some(body) = else_branch {
                    self.line("} else {");
                    self.indent += 1;
                    self.statements(body);
                    self.indent -= 1;
                }
                self.line("}");
            }
            Statement::While { condition, body } => {
                self.line(&format!("while {} {{", self.expression(condition)));
                self.indent += 1;
                self.statements(body);
                self.indent -= 1;
                self.line("}");
            }
            Statement::Loop { body } => {
                self.line("loop {");
                self.indent += 1;
                self.statements(body);
                self.indent -= 1;
                self.line("}");
            }
            Statement::Repeat { count, index, body } => {
                let header = match index {
                    Some(index) => format!("repeat {} as {index} {{", self.expression(count)),
                    None => format!("repeat {} {{", self.expression(count)),
                };
                self.line(&header);
                self.indent += 1;
                self.statements(body);
                self.indent -= 1;
                self.line("}");
            }
            Statement::Each {
                names,
                iterable,
                body,
            } => {
                self.line(&format!(
                    "each {} in {} {{",
                    names.join(", "),
                    self.expression(iterable)
                ));
                self.indent += 1;
                self.statements(body);
                self.indent -= 1;
                self.line("}");
            }
            Statement::Break => self.line("break"),
            Statement::Continue => self.line("continue"),
            Statement::Return(value) => match value {
                Some(value) => self.line(&format!("return {}", self.expression(value))),
                None => self.line("return"),
            },
            Statement::Expression(expression) => {
                self.line(&self.expression(expression));
            }
        }
    }

    fn expression(&self, expression: &Expression) -> String {
        match expression {
            Expression::Literal(literal) => literal.text(),
            Expression::Name(name) => name.clone(),
            Expression::Field { target, name } => {
                format!("{}.{}", self.atom(target), name)
            }
            Expression::Index { target, index } => {
                format!("{}[{}]", self.atom(target), self.expression(index))
            }
            Expression::Unary { operator, operand } => {
                format!("{}({})", operator.symbol(), self.expression(operand))
            }
            Expression::Binary {
                operator,
                left,
                right,
            } => format!(
                "({} {} {})",
                self.expression(left),
                operator.symbol(),
                self.expression(right)
            ),
            Expression::Call {
                function,
                arguments,
            } => {
                let arguments: Vec<String> = arguments
                    .iter()
                    .map(|value| self.expression(value))
                    .collect();
                format!("{}({})", self.atom(function), arguments.join(", "))
            }
            Expression::List(values) => {
                let values: Vec<String> =
                    values.iter().map(|value| self.expression(value)).collect();
                format!("[{}]", values.join(", "))
            }
            Expression::Dict(entries) => {
                let entries: Vec<String> = entries
                    .iter()
                    .map(|(key, value)| {
                        format!("{}: {}", self.expression(key), self.expression(value))
                    })
                    .collect();
                format!("{{{}}}", entries.join(", "))
            }
        }
    }

    fn atom(&self, expression: &Expression) -> String {
        match expression {
            Expression::Literal(_)
            | Expression::Name(_)
            | Expression::Field { .. }
            | Expression::Index { .. }
            | Expression::Call { .. }
            | Expression::List(_)
            | Expression::Dict(_) => self.expression(expression),
            Expression::Unary { .. } | Expression::Binary { .. } => {
                format!("({})", self.expression(expression))
            }
        }
    }
}

impl Literal {
    fn text(&self) -> String {
        match self {
            Literal::Integer(value) => value.to_string(),
            Literal::Float(value) => {
                let text = format!("{value:?}");
                if text.contains('.') || text.contains('e') || text.contains('E') {
                    text
                } else {
                    format!("{text}.0")
                }
            }
            Literal::Bool(true) => "true".to_string(),
            Literal::Bool(false) => "false".to_string(),
            Literal::Text(value) => format!("\"{}\"", escape_text(value)),
            Literal::None => "none".to_string(),
        }
    }
}

fn escape_text(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

impl UnaryOperator {
    fn symbol(self) -> &'static str {
        match self {
            Self::Negate => "-",
            Self::Positive => "+",
            Self::Not => "not ",
        }
    }
}

impl BinaryOperator {
    fn symbol(self) -> &'static str {
        match self {
            Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::IntegerDivide => "//",
            Self::Remainder => "%",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::Less => "<",
            Self::LessEqual => "<=",
            Self::Greater => ">",
            Self::GreaterEqual => ">=",
            Self::And => "and",
            Self::Or => "or",
        }
    }
}

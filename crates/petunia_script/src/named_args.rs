//! Host calls lose argument labels in Aipo's IR. Normalize checked HIR instead.
//! Reordered calls use an IIFE: evaluate the callee and every argument once, in source order,
//! then pass those values in schema order. No instruction or jump offsets are moved.

use aipo_diagnostics::{Diagnostic, DiagnosticCode};
use aipo_hir::{
    HirCallArg, HirExpr, HirFunctionDecl, HirFunctionExpr, HirItem, HirMatchPattern, HirParam,
    HirProgram, HirStmt, HirVariantPatternPayload,
};
use aipo_sema::PreludeSurface;
use aipo_source::{Source, SourceSpan};

pub(super) fn desugar_named_args(
    program: &mut HirProgram,
    surface: &PreludeSurface,
    source: &Source,
) -> Vec<Diagnostic> {
    let mut pass = NamedArguments {
        surface,
        source,
        bindings: Vec::new(),
        diagnostics: Vec::new(),
    };
    // Module declarations are visible to every function, including functions written earlier.
    for item in &program.items {
        match item {
            HirItem::Fn(function) => pass.bindings.push(function.name.clone()),
            HirItem::Struct(declaration) => pass.bindings.push(declaration.name.clone()),
            HirItem::Enum(declaration) => pass.bindings.push(declaration.name.clone()),
            HirItem::Interface(declaration) => pass.bindings.push(declaration.name.clone()),
            HirItem::Import(import) => {
                pass.bindings
                    .push(import.alias.as_ref().unwrap_or(&import.module_name).clone());
                pass.bindings.extend(import.names.iter().cloned());
            }
            HirItem::Impl(_) | HirItem::Batch(_) | HirItem::Export(_) => {}
        }
    }
    for statement in &program.statements {
        if let HirStmt::Let(name, ..) | HirStmt::Var(name, ..) = statement {
            pass.bindings.push(name.clone());
        }
    }
    for item in &mut program.items {
        match item {
            HirItem::Fn(function) => pass.function(function),
            HirItem::Struct(declaration) => {
                for field in &mut declaration.fields {
                    if let Some(default) = &mut field.default {
                        pass.expression(default);
                    }
                }
            }
            HirItem::Impl(implementation) => {
                for function in implementation
                    .init
                    .iter_mut()
                    .chain(implementation.invariant.iter_mut())
                    .chain(implementation.methods.iter_mut())
                {
                    pass.function(function);
                }
            }
            HirItem::Interface(declaration) => {
                for function in &mut declaration.methods {
                    pass.function(function);
                }
            }
            HirItem::Enum(_) | HirItem::Batch(_) | HirItem::Import(_) | HirItem::Export(_) => {}
        }
    }
    pass.statements(&mut program.statements);
    pass.diagnostics
}

struct NamedArguments<'source> {
    surface: &'source PreludeSurface,
    source: &'source Source,
    // A stack of lexical names; block exit truncates only that block's bindings.
    bindings: Vec<String>,
    diagnostics: Vec<Diagnostic>,
}

impl NamedArguments<'_> {
    fn reject(&mut self, span: SourceSpan, reason: &str) {
        self.diagnostics.push(
            Diagnostic::error(DiagnosticCode::AIPO_SEM_NAMED_ARG_UNKNOWN, reason)
                .with_primary_span(self.source, span),
        );
    }

    fn function(&mut self, function: &mut HirFunctionDecl) {
        self.function_body(&mut function.params, &mut function.body);
    }

    fn function_body(&mut self, parameters: &mut [HirParam], body: &mut [HirStmt]) {
        let enclosing_bindings = self.bindings.len();
        self.bindings
            .extend(parameters.iter().map(|parameter| parameter.name.clone()));
        for parameter in parameters {
            if let Some(default) = &mut parameter.default {
                self.expression(default);
            }
        }
        self.statements(body);
        self.bindings.truncate(enclosing_bindings);
    }

    fn block(&mut self, body: &mut [HirStmt]) {
        let enclosing_bindings = self.bindings.len();
        self.statements(body);
        self.bindings.truncate(enclosing_bindings);
    }

    fn statements(&mut self, statements: &mut [HirStmt]) {
        for statement in statements {
            match statement {
                HirStmt::Let(name, value, _) | HirStmt::Var(name, value, _) => {
                    self.expression(value);
                    self.bindings.push(name.clone());
                }
                HirStmt::Assign(target, value, _)
                | HirStmt::CompoundAssign(_, target, value, _) => {
                    self.expression(target);
                    self.expression(value);
                }
                HirStmt::If(branches) => {
                    self.expression(&mut branches.condition);
                    self.block(&mut branches.then_branch);
                    for (condition, body) in &mut branches.elif_branches {
                        self.expression(condition);
                        self.block(body);
                    }
                    if let Some(body) = &mut branches.else_branch {
                        self.block(body);
                    }
                }
                HirStmt::Match(branches) => {
                    self.expression(&mut branches.target);
                    for arm in &mut branches.when_arms {
                        let enclosing_bindings = self.bindings.len();
                        for pattern in &mut arm.patterns {
                            match pattern {
                                HirMatchPattern::Value(value) => self.expression(value),
                                HirMatchPattern::Destructure(names)
                                | HirMatchPattern::Variant {
                                    payload:
                                        HirVariantPatternPayload::Tuple(names)
                                        | HirVariantPatternPayload::Struct(names),
                                    ..
                                } => self.bindings.extend(names.iter().cloned()),
                                HirMatchPattern::Variant {
                                    payload: HirVariantPatternPayload::Unit,
                                    ..
                                } => {}
                            }
                        }
                        if let Some(guard) = &mut arm.guard {
                            self.expression(guard);
                        }
                        self.statements(&mut arm.body);
                        self.bindings.truncate(enclosing_bindings);
                    }
                    if let Some(body) = &mut branches.else_arm {
                        self.block(body);
                    }
                }
                HirStmt::Loop(body, _) | HirStmt::AwaitDo(body, _) => self.block(body),
                HirStmt::While(condition, body, _) => {
                    self.expression(condition);
                    self.block(body);
                }
                HirStmt::Repeat(count, alias, body, _) => {
                    self.expression(count);
                    let enclosing_bindings = self.bindings.len();
                    self.bindings.extend(alias.iter().cloned());
                    self.statements(body);
                    self.bindings.truncate(enclosing_bindings);
                }
                HirStmt::Each(names, iterable, body, _) => {
                    self.expression(iterable);
                    let enclosing_bindings = self.bindings.len();
                    self.bindings.extend(names.iter().cloned());
                    self.statements(body);
                    self.bindings.truncate(enclosing_bindings);
                }
                HirStmt::Return(value, _) => {
                    if let Some(value) = value {
                        self.expression(value);
                    }
                }
                HirStmt::Fail(value, _) | HirStmt::Expr(value) => self.expression(value),
                HirStmt::Attempt(attempt) => {
                    self.block(&mut attempt.body);
                    let enclosing_bindings = self.bindings.len();
                    self.bindings.extend(attempt.error_binding.iter().cloned());
                    self.statements(&mut attempt.handler);
                    self.bindings.truncate(enclosing_bindings);
                }
                HirStmt::FnDecl(function) => {
                    self.bindings.push(function.name.clone());
                    self.function(function);
                }
                HirStmt::Break(_) | HirStmt::Continue(_) => {}
            }
        }
    }

    fn expression(&mut self, expression: &mut HirExpr) {
        match expression {
            HirExpr::Call(callee, arguments, span) => {
                self.expression(callee);
                for argument in arguments.iter_mut() {
                    self.expression(&mut argument.value);
                }
                if arguments.iter().any(|argument| argument.name.is_some()) {
                    let span = *span;
                    if let Some(order) = self.argument_order(callee, arguments, span) {
                        if order.iter().copied().eq(0..arguments.len()) {
                            for argument in arguments {
                                argument.name = None;
                            }
                        } else {
                            *expression = ordered_call(callee, arguments, &order, span);
                        }
                    }
                }
            }
            HirExpr::Unary(_, inner, _)
            | HirExpr::Dot(inner, _, _)
            | HirExpr::QuestionDot(inner, _, _)
            | HirExpr::Await(inner, _)
            | HirExpr::Try(inner, _) => self.expression(inner),
            HirExpr::Binary(_, left, right, _)
            | HirExpr::Index(left, right, _)
            | HirExpr::OrElse(left, right, _) => {
                self.expression(left);
                self.expression(right);
            }
            HirExpr::List(values, _) => {
                for value in values {
                    self.expression(value);
                }
            }
            HirExpr::Dict(entries, _) => {
                for (key, value) in entries {
                    self.expression(key);
                    self.expression(value);
                }
            }
            HirExpr::Construct(_, fields, _) => {
                for (_, value) in fields {
                    self.expression(value);
                }
            }
            HirExpr::With(base, fields, _) => {
                self.expression(base);
                for (_, value) in fields {
                    self.expression(value);
                }
            }
            HirExpr::Fn(function) => self.function_body(&mut function.params, &mut function.body),
            HirExpr::If(condition, then_value, else_value, _) => {
                self.expression(condition);
                self.expression(then_value);
                self.expression(else_value);
            }
            HirExpr::Literal(_, _) | HirExpr::Identifier(_, _) => {}
        }
    }

    fn argument_order(
        &mut self,
        callee: &HirExpr,
        arguments: &[HirCallArg],
        span: SourceSpan,
    ) -> Option<Vec<usize>> {
        let HirExpr::Dot(base, member, _) = callee else {
            if matches!(callee, HirExpr::QuestionDot(..)) {
                self.reject(
                    span,
                    "named arguments on safe-navigation calls are unsupported",
                );
            }
            // Ordinary function calls remain the language compiler's responsibility.
            return None;
        };
        let signature = match &**base {
            HirExpr::Identifier(module, _) if !self.bindings.contains(module) => {
                self.surface.host_function(module, member)
            }
            _ => None,
        };
        let Some(signature) = signature else {
            self.reject(
                span,
                "named arguments require an unshadowed host module function",
            );
            return None;
        };
        let mut targets = Vec::with_capacity(arguments.len());
        let mut named_seen = false;
        for (position, argument) in arguments.iter().enumerate() {
            let target = if let Some(name) = &argument.name {
                named_seen = true;
                signature
                    .params
                    .iter()
                    .position(|parameter| &parameter.name == name)
            } else if !named_seen && (position < signature.params.len() || signature.variadic) {
                Some(position)
            } else {
                None
            };
            let Some(target) = target else {
                self.reject(
                    argument.span,
                    "cannot bind this argument to the host signature",
                );
                return None;
            };
            if targets.contains(&target) {
                self.reject(argument.span, "host parameter supplied more than once");
                return None;
            }
            targets.push(target);
        }
        // ponytail: fixed required parameters only; optional omissions need explicit unset HIR.
        if (0..signature.params.len()).any(|parameter| !targets.contains(&parameter)) {
            self.reject(
                span,
                "named host calls must supply every declared parameter",
            );
            return None;
        }
        let mut order: Vec<usize> = (0..arguments.len()).collect();
        order.sort_by_key(|position| targets.get(*position).copied());
        Some(order)
    }
}

fn ordered_call(
    callee: &HirExpr,
    arguments: &mut Vec<HirCallArg>,
    order: &[usize],
    span: SourceSpan,
) -> HirExpr {
    // '$' cannot occur in source identifiers. Generated names cannot capture guest bindings.
    let callee_name = "$petunia_named_callee".to_string();
    let argument_name = |position| format!("$petunia_named_argument_{position}");
    let parameters = std::iter::once(callee_name.clone())
        .chain((0..arguments.len()).map(argument_name))
        .map(|name| HirParam {
            name,
            is_mut: false,
            is_self: false,
            type_annotation: None,
            default: None,
            span,
        })
        .collect();
    let forwarded = order
        .iter()
        .map(|position| HirCallArg {
            name: None,
            value: HirExpr::Identifier(argument_name(*position), span),
            span,
        })
        .collect();
    let mut evaluated = vec![HirCallArg {
        name: None,
        value: callee.clone(),
        span: callee.span(),
    }];
    evaluated.extend(std::mem::take(arguments).into_iter().map(|mut argument| {
        argument.name = None;
        argument
    }));
    HirExpr::Call(
        Box::new(HirExpr::Fn(HirFunctionExpr {
            is_async: false,
            params: parameters,
            return_type: None,
            body: vec![HirStmt::Return(
                Some(HirExpr::Call(
                    Box::new(HirExpr::Identifier(callee_name, span)),
                    forwarded,
                    span,
                )),
                span,
            )],
            span,
        })),
        evaluated,
        span,
    )
}

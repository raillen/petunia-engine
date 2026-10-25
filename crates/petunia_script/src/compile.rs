//! Compiling a script: parse, lower, check against the API surface, lower to IR, emit bytecode.
//! Every problem comes back as a [`ScriptDiagnostic`] with a line and column, so the editor can
//! underline it and the Project Doctor can list it.

use aipo_bytecode::BytecodeModule;
use aipo_diagnostics::{Diagnostic, Severity};
use aipo_sema::PreludeSurface;
use aipo_source::{Source, SourceId};

#[path = "named_args.rs"]
mod named_args;

/// A script is game logic, not a data file.
pub const MAX_SCRIPT_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptSeverity {
    Error,
    Warning,
    Info,
}

impl ScriptSeverity {
    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// One problem found in a script. Lines and columns start at 1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptDiagnostic {
    pub severity: ScriptSeverity,
    pub code: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub notes: Vec<String>,
}

impl ScriptDiagnostic {
    /// A problem with no place in the text (too big, or a failure while running it).
    pub fn general(code: &str, message: impl Into<String>) -> Self {
        Self {
            severity: ScriptSeverity::Error,
            code: code.to_string(),
            message: message.into(),
            line: 1,
            column: 1,
            end_line: 1,
            end_column: 1,
            notes: Vec::new(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == ScriptSeverity::Error
    }
}

/// The result of compiling: a module when there are no errors, and every diagnostic either way.
pub struct Compiled {
    pub module: Option<BytecodeModule>,
    pub diagnostics: Vec<ScriptDiagnostic>,
}

fn convert(source: &Source, diagnostic: &Diagnostic) -> ScriptDiagnostic {
    let severity = match diagnostic.severity {
        Severity::Error | Severity::Fault => ScriptSeverity::Error,
        Severity::Warning => ScriptSeverity::Warning,
        Severity::Note => ScriptSeverity::Info,
    };
    let (line, column, end_line, end_column) = match &diagnostic.primary_span {
        Some(span) => {
            let end = source
                .location(span.end.max(span.start))
                .map_or((span.line, span.column + 1), |at| (at.line, at.column));
            (span.line, span.column, end.0, end.1)
        }
        None => (1, 1, 1, 1),
    };
    ScriptDiagnostic {
        severity,
        code: diagnostic.code.to_string(),
        message: diagnostic.message.clone(),
        line,
        column,
        end_line,
        end_column,
        notes: diagnostic.notes.clone(),
    }
}

/// Compiles `text` as the script called `name`.
pub fn compile(name: &str, text: &str, surface: &PreludeSurface) -> Compiled {
    if text.len() > MAX_SCRIPT_BYTES {
        return Compiled {
            module: None,
            diagnostics: vec![ScriptDiagnostic::general(
                "SCRIPT_TOO_BIG",
                format!(
                    "o script tem {} bytes; o máximo é {MAX_SCRIPT_BYTES}",
                    text.len()
                ),
            )],
        };
    }
    let source = Source::new(SourceId::next(), name, text);
    let (ast, mut raw) = aipo_syntax::parse(&source);
    let mut failed = raw
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error);
    let mut module = None;
    if !failed {
        let mut hir = aipo_hir::lower(ast);
        let (_, semantic) = aipo_sema::check_with_prelude(&source, &hir, surface);
        failed = semantic
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        raw.extend(semantic);
        if !failed {
            let desugared = named_args::desugar_named_args(&mut hir, surface, &source);
            failed = desugared
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error);
            raw.extend(desugared);
            if !failed {
                let ir = aipo_ir::lower_to_ir(&hir);
                match aipo_bytecode::compile(&ir) {
                    Ok(compiled) => module = Some(compiled),
                    Err(errors) => {
                        failed = true;
                        for reason in errors {
                            let fault = aipo_vm::VmFault::CorruptedBytecode { offset: 0, reason };
                            raw.push(Diagnostic::error(
                                fault.diagnostic_code(),
                                fault.to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }
    let mut diagnostics: Vec<ScriptDiagnostic> = raw
        .iter()
        .map(|diagnostic| convert(&source, diagnostic))
        .collect();
    diagnostics.sort_by_key(|diagnostic| (diagnostic.line, diagnostic.column));
    Compiled {
        module: if failed { None } else { module },
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::script_surface;
    use petunia_commands::{CommandRecord, Dispatcher};
    use petunia_project::register_standard_tables;
    use petunia_world::{PersistentId, Value, World};
    use serde_json::json;

    #[test]
    fn a_good_script_compiles_with_no_errors() {
        let compiled = compile(
            "ok.aipo",
            "fn on_update(entity, dt) {\n    log.info(\"oi\", entity)\n}\n",
            &script_surface(),
        );
        assert!(compiled.module.is_some(), "{:?}", compiled.diagnostics);
        assert!(compiled.diagnostics.iter().all(|d| !d.is_error()));
    }

    #[test]
    fn mistakes_come_back_with_a_line_and_column() {
        let surface = script_surface();
        let syntax = compile(
            "a.aipo",
            "fn on_start(entity) {\n    let = 3\n}\n",
            &surface,
        );
        assert!(syntax.module.is_none());
        assert_eq!(syntax.diagnostics.first().map(|d| d.line), Some(2));
        let typo = compile(
            "b.aipo",
            "fn on_start(entity) {\n    world.fnd(\"Brush\")\n}\n",
            &surface,
        );
        assert!(typo.module.is_none(), "an API typo is a compile error");
        assert!(
            typo.diagnostics.iter().any(|d| d.line == 2 && d.is_error()),
            "{:?}",
            typo.diagnostics
        );
    }

    #[test]
    fn modules_a_game_must_not_use_do_not_compile() {
        let surface = script_surface();
        for call in [
            "sh.run(\"ls\")",
            "fs.read_text(\"/etc/passwd\")",
            "env.get(\"HOME\")",
            "io.println(\"x\")",
        ] {
            let compiled = compile(
                "c.aipo",
                &format!("fn on_start(entity) {{\n    {call}\n}}\n"),
                &surface,
            );
            assert!(compiled.module.is_none(), "`{call}` must not compile");
        }
    }

    #[test]
    fn a_huge_script_is_refused_before_parsing() {
        let compiled = compile(
            "big.aipo",
            &"#".repeat(MAX_SCRIPT_BYTES + 1),
            &script_surface(),
        );
        assert!(compiled.module.is_none());
        assert_eq!(
            compiled.diagnostics.first().map(|d| d.code.as_str()),
            Some("SCRIPT_TOO_BIG")
        );
    }

    fn test_world_with(script_name: &str) -> (World, u64) {
        let mut world = World::new(7);
        register_standard_tables(&mut world).expect("tables");
        let mut dispatcher = Dispatcher::with_builtin().expect("dispatcher");
        let created = dispatcher
            .execute(
                &mut world,
                &CommandRecord::new(
                    "world.create",
                    json!({ "rows": {
                        "Name": { "text": "Target" },
                        "Transform": { "position": { "x": 0.0, "y": 0.0, "z": 0.0 } },
                        "Script": { "file": script_name, "enabled": true }
                    }}),
                ),
            )
            .expect("create");
        let id = created["entity"].as_u64().expect("entity id");
        (world, id)
    }

    fn position_x_of(host: &crate::host::ScriptHost, id: u64) -> f64 {
        let world = host.world();
        let handle = world.handle_of(PersistentId(id)).expect("entity");
        match world
            .get(handle, "Transform", "position")
            .expect("position")
        {
            Value::Vec3(pos) => f64::from(pos.x),
            other => panic!("expected Vec3, found {other:?}"),
        }
    }

    #[test]
    fn named_args_in_declared_order_execute_correctly() {
        let (world, id) = test_world_with("in_order");
        let mut host = crate::host::ScriptHost::new(world).expect("host");
        host.load(
            "in_order",
            r#"
fn on_start(entity) {
    world.set(
        entity = entity,
        table = "Transform",
        column = "position",
        value = { "x": 10.0, "y": 0.0, "z": 0.0 },
    )
}
"#,
        )
        .expect("compiles");
        host.update(0.016);
        assert!(host.take_faults().is_empty());
        assert_eq!(host.commands_run(), 1);
        assert!((position_x_of(&host, id) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn reordered_named_args_evaluate_left_to_right_and_bind_correctly() {
        let (world, id) = test_world_with("reordered");
        let mut host = crate::host::ScriptHost::new(world).expect("host");
        host.load(
            "reordered",
            r#"
fn make_val(entity) {
    log.info("evaluated_value")
    return { "x": 42.0, "y": 0.0, "z": 0.0 }
}

fn make_col() {
    log.info("evaluated_column")
    return "position"
}

fn on_start(entity) {
    world.set(
        entity = entity,
        value = make_val(entity),
        column = make_col(),
        table = "Transform",
    )
}
"#,
        )
        .expect("compiles");
        host.update(0.016);
        assert!(host.take_faults().is_empty());
        assert_eq!(host.commands_run(), 1);
        assert!((position_x_of(&host, id) - 42.0).abs() < 1e-6);
        let logs: Vec<String> = host.take_log().into_iter().map(|l| l.text).collect();
        assert_eq!(
            logs,
            vec![
                "evaluated_value".to_string(),
                "evaluated_column".to_string()
            ]
        );
    }

    #[test]
    fn named_args_on_parameter_shadowed_host_module_are_rejected() {
        let surface = script_surface();
        let compiled = compile(
            "shadow_param.aipo",
            r#"
fn on_start(world) {
    world.set(entity = 1, table = "Transform", column = "position", value = 0)
}
"#,
            &surface,
        );
        assert!(compiled.module.is_none());
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|d| d.is_error() && d.code == "AIPO_SEM_NAMED_ARG_UNKNOWN"),
            "{:?}",
            compiled.diagnostics
        );
    }

    #[test]
    fn named_args_on_local_shadowed_host_module_are_rejected() {
        let surface = script_surface();
        let compiled = compile(
            "shadow_local.aipo",
            r#"
fn on_start(entity) {
    let world = 99
    world.set(entity = entity, table = "Transform", column = "position", value = 0)
}
"#,
            &surface,
        );
        assert!(compiled.module.is_none());
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|d| d.is_error() && d.code == "AIPO_SEM_NAMED_ARG_UNKNOWN"),
            "{:?}",
            compiled.diagnostics
        );
    }

    #[test]
    fn named_args_on_block_shadowed_host_module_are_rejected() {
        let surface = script_surface();
        let compiled = compile(
            "shadow_block.aipo",
            r#"
fn on_start(entity) {
    if true {
        let world = entity
        world.set(entity = entity, table = "Transform", column = "position", value = 0)
    }
}
"#,
            &surface,
        );
        assert!(compiled.module.is_none());
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|d| d.is_error() && d.code == "AIPO_SEM_NAMED_ARG_UNKNOWN"),
            "{:?}",
            compiled.diagnostics
        );
    }

    #[test]
    fn named_args_on_unsupported_dot_call_are_rejected() {
        let surface = script_surface();
        let compiled = compile(
            "unsupported_dot.aipo",
            r#"
fn on_start(entity) {
    entity.foo(bar = 1)
}
"#,
            &surface,
        );
        assert!(compiled.module.is_none());
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|d| d.is_error() && d.code == "AIPO_SEM_NAMED_ARG_UNKNOWN"),
            "{:?}",
            compiled.diagnostics
        );
    }
}

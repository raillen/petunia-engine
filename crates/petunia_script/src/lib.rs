//! The Script Host. Aipo is the language of a project's scripts (DR-06, DR-25); this crate compiles
//! them, runs their handlers against the world, and describes the API they see as data.
//!
//! - [`compile`] gives diagnostics with line and column (editor squiggles, Project Doctor).
//! - [`ScriptHost`] runs scripts: one VM per object-script attachment (the compiled module is
//!   shared), capabilities deny by default, an instruction budget per call, a faulty object's
//!   script switched off instead of spamming the console.
//! - [`api`] is the single table behind the compiler surface, the natives and the editor help.
//! - [`simulate`] tries a script on a copy of the world (the editor's "Run").
//!
//! A script may use the world (`world`), the game clock and events (`game`), the console (`log`),
//! and Aipo's pure standard library. It cannot run programs, read files or the environment: those
//! modules are removed, not just denied (see `REMOVED_MODULES`).

mod api;
mod compile;
mod convert;
mod doctor;
mod gameplay;
mod graph;
mod grid_api;
mod host;
pub mod ir_graph;
mod natives;
mod physics_api;
pub mod source_ir;

pub use api::{API, ApiFunction, GAME_CAPABILITIES, HANDLERS, KEYWORDS, api_json, script_surface};
pub use compile::{Compiled, MAX_SCRIPT_BYTES, ScriptDiagnostic, ScriptSeverity, compile};
pub use doctor::{project_diagnostics, script_names};
pub use graph::{
    GRAPH_EVENTS, GRAPH_SUFFIX, GraphEvent, blame_node, compile_graph, compile_graph_with,
    graph_catalog, graph_max_bytes, graph_names,
};
pub use host::{
    LoadReport, PlayerRig, RunReport, SCRIPT_EXTENSION, SCRIPTS_DIRECTORY, ScriptFault, ScriptHost,
    ScriptLimits, simulate,
};
pub use natives::{LogLevel, LogLine};
pub use petunia_physics::PhysicsSettings;
pub use physics_api::PlayerInput;

/// Formats a script's text the way `aipo fmt` does. The text must parse first (the formatter is
/// forgiving about broken text, and silently "formatting" a mistake would hide it): `Err` carries
/// the first syntax error with its line.
pub fn format_script(name: &str, text: &str) -> Result<String, String> {
    let source = aipo_source::Source::new(aipo_source::SourceId::next(), name, text);
    let (_, diagnostics) = aipo_syntax::parse(&source);
    if let Some(error) = diagnostics
        .iter()
        .find(|d| d.severity == aipo_diagnostics::Severity::Error)
    {
        let line = error.primary_span.as_ref().map_or(1, |span| span.line);
        return Err(format!("linha {line}: {}", error.message));
    }
    aipo_formatter::format_text(name, text).map_err(|error| error.to_string())
}

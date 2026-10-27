//! Node graphs as scripts (DR-30). A file `scripts/<name>.graph.json` is a graph; the host turns it
//! into Aipo text in memory each time it loads, so a generated file can never be stale. The node
//! catalog is generated from the same API table the compiler and the editor help use.

use std::path::Path;
use std::sync::OnceLock;

use petunia_graph::{
    ApiSpec, Catalog, Compiled, Diagnostic, EventSpec, GraphDocument, Severity as GraphSeverity,
};

use crate::api::API;
use crate::compile::{ScriptDiagnostic, ScriptSeverity};

/// A graph file ends with this; the name before it is the script name.
pub const GRAPH_SUFFIX: &str = ".graph.json";

/// A handler a graph can start from, with its typed parameters. Every handler must also be in
/// `HANDLERS` (a test checks).
pub struct GraphEvent {
    pub handler: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub params: &'static [(&'static str, &'static str)],
}

pub const GRAPH_EVENTS: &[GraphEvent] = &[
    GraphEvent {
        handler: "on_start",
        title: "Ao começar",
        summary: "Uma vez, antes da primeira atualização do objeto.",
        params: &[("entity", "Int")],
    },
    GraphEvent {
        handler: "on_update",
        title: "A cada quadro",
        summary: "Todo quadro; 'dt' são os segundos desde o quadro anterior.",
        params: &[("entity", "Int"), ("dt", "Float")],
    },
    GraphEvent {
        handler: "on_event",
        title: "Ao receber evento",
        summary: "Quando algo emite um evento com este objeto.",
        params: &[("entity", "Int"), ("name", "String")],
    },
    GraphEvent {
        handler: "on_collision",
        title: "Ao começar a tocar",
        summary: "Uma colisão deste objeto começou a tocar outra.",
        params: &[
            ("entity", "Int"),
            ("other", "Int"),
            ("mine", "String"),
            ("theirs", "String"),
        ],
    },
    GraphEvent {
        handler: "on_collision_end",
        title: "Ao parar de tocar",
        summary: "As duas colisões deixaram de se tocar.",
        params: &[
            ("entity", "Int"),
            ("other", "Int"),
            ("mine", "String"),
            ("theirs", "String"),
        ],
    },
];

/// The nodes a graph may use, built once.
pub fn graph_catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let api: Vec<ApiSpec> = API
            .iter()
            .map(|function| ApiSpec {
                module: function.module.to_string(),
                name: function.name.to_string(),
                params: function
                    .params
                    .iter()
                    .map(|(name, kind)| (name.to_string(), kind.to_string()))
                    .collect(),
                returns: function.returns.to_string(),
                summary: function.summary.to_string(),
                permission: function.permission.to_string(),
            })
            .collect();
        let events: Vec<EventSpec> = GRAPH_EVENTS
            .iter()
            .map(|event| EventSpec {
                handler: event.handler.to_string(),
                title: event.title.to_string(),
                summary: event.summary.to_string(),
                params: event
                    .params
                    .iter()
                    .map(|(name, kind)| (name.to_string(), kind.to_string()))
                    .collect(),
            })
            .collect();
        Catalog::new(&api, &events)
    })
}

/// Names of the graph files in `directory`, sorted.
pub fn graph_names(directory: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().into_owned();
            file.strip_suffix(GRAPH_SUFFIX).map(str::to_string)
        })
        .filter(|name| !name.is_empty())
        .collect();
    names.sort();
    names
}

fn to_script(finding: &Diagnostic) -> ScriptDiagnostic {
    let mut diagnostic = ScriptDiagnostic::general(finding.code, finding.message.clone());
    diagnostic.severity = match finding.severity {
        GraphSeverity::Error => ScriptSeverity::Error,
        GraphSeverity::Warning => ScriptSeverity::Warning,
    };
    diagnostic
}

/// Parses and compiles a graph's text. Errors come as script diagnostics (a sentence aimed at a
/// node, not at a line of generated text).
pub fn compile_graph(text: &str) -> Result<Compiled, Vec<ScriptDiagnostic>> {
    let document = GraphDocument::parse(text)
        .map_err(|error| vec![ScriptDiagnostic::general("GRAPH_INVALID", error.0)])?;
    petunia_graph::compile(&document, graph_catalog())
        .map_err(|findings| findings.iter().map(to_script).collect())
}

/// Turns a diagnostic found in the generated Aipo into one about the node that wrote the line.
pub fn blame_node(compiled: &Compiled, diagnostic: &ScriptDiagnostic) -> ScriptDiagnostic {
    let node = diagnostic
        .line
        .checked_sub(1)
        .and_then(|index| compiled.line_nodes.get(index))
        .copied()
        .flatten();
    let mut blamed = diagnostic.clone();
    if let Some(node) = node {
        blamed.message = format!("nó {node}: {}", diagnostic.message);
    }
    blamed
}

pub(crate) fn warnings_of(compiled: &Compiled) -> Vec<ScriptDiagnostic> {
    compiled.warnings.iter().map(to_script).collect()
}

/// The most bytes a graph file may have.
pub fn graph_max_bytes() -> usize {
    petunia_graph::MAX_FILE_BYTES
}

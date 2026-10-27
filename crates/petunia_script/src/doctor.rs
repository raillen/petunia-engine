//! What the Project Doctor says about a project's scripts: files that do not compile, files that
//! cannot be read, and objects that point at a script that is not there. A broken script never
//! stops a project from opening; it is listed here, for the editor and for `petunia check`.

use std::path::Path;

use petunia_project::{Diagnostic, Severity};
use petunia_world::{Value, World};

use crate::api::script_surface;
use crate::compile::{MAX_SCRIPT_BYTES, ScriptSeverity, compile};
use crate::graph::{GRAPH_SUFFIX, compile_graph, graph_names};
use crate::host::{SCRIPT_EXTENSION, SCRIPTS_DIRECTORY};

/// Names of the valid script files in `directory`, sorted.
pub fn script_names(directory: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(SCRIPT_EXTENSION))
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
        })
        .collect();
    names.sort();
    names
}

/// The findings about the scripts of the project at `root`, whose scene is `world`.
pub fn project_diagnostics(root: &Path, world: &World) -> Vec<Diagnostic> {
    let directory = root.join(SCRIPTS_DIRECTORY);
    let surface = script_surface();
    let names = script_names(&directory);
    let mut found = Vec::new();
    for name in &names {
        let file = format!("{name}.{SCRIPT_EXTENSION}");
        let path = directory.join(&file);
        let text = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() as usize > MAX_SCRIPT_BYTES => {
                found.push(Diagnostic::error(
                    "SCRIPT_TOO_BIG",
                    &format!("{SCRIPTS_DIRECTORY}/{file}"),
                    &format!("o arquivo tem mais de {MAX_SCRIPT_BYTES} bytes"),
                    "Divida o script em dois.",
                ));
                continue;
            }
            _ => std::fs::read_to_string(&path),
        };
        let text = match text {
            Ok(text) => text,
            Err(error) => {
                found.push(Diagnostic::error(
                    "SCRIPT_UNREADABLE",
                    &format!("{SCRIPTS_DIRECTORY}/{file}"),
                    &format!("não consegui ler o script: {error}"),
                    "Confira se o arquivo é texto UTF-8 e se pode ser lido.",
                ));
                continue;
            }
        };
        for diagnostic in compile(&file, &text, &surface).diagnostics {
            let severity = match diagnostic.severity {
                ScriptSeverity::Error => Severity::Error,
                ScriptSeverity::Warning => Severity::Warning,
                ScriptSeverity::Info => Severity::Info,
            };
            found.push(Diagnostic {
                severity,
                code: diagnostic.code,
                target: format!(
                    "{SCRIPTS_DIRECTORY}/{file}:{}:{}",
                    diagnostic.line, diagnostic.column
                ),
                message: diagnostic.message,
                action: diagnostic.notes.first().cloned().unwrap_or_else(|| {
                    "Abra o script (editor de código, F4) e corrija o trecho indicado pela linha e coluna.".to_string()
                }),
            });
        }
    }
    let graphs = graph_names(&directory);
    for name in &graphs {
        let file = format!("{name}{GRAPH_SUFFIX}");
        let target = format!("{SCRIPTS_DIRECTORY}/{file}");
        let path = directory.join(&file);
        if names.contains(name) {
            found.push(Diagnostic::error(
                "GRAPH_NAME_CLASH",
                &target,
                &format!("existem `{name}.{SCRIPT_EXTENSION}` e `{file}` com o mesmo nome"),
                "Apague ou renomeie um dos dois.",
            ));
            continue;
        }
        let text = match std::fs::metadata(&path) {
            Ok(meta) if meta.len() as usize > petunia_graph::MAX_FILE_BYTES => {
                found.push(Diagnostic::error(
                    "GRAPH_TOO_BIG",
                    &target,
                    &format!(
                        "o grafo tem mais de {} bytes",
                        petunia_graph::MAX_FILE_BYTES
                    ),
                    "Divida a lógica em dois grafos.",
                ));
                continue;
            }
            _ => std::fs::read_to_string(&path),
        };
        let text = match text {
            Ok(text) => text,
            Err(error) => {
                found.push(Diagnostic::error(
                    "GRAPH_UNREADABLE",
                    &target,
                    &format!("não consegui ler o grafo: {error}"),
                    "Confira se o arquivo é texto UTF-8 e se pode ser lido.",
                ));
                continue;
            }
        };
        let (diagnostics, is_error) = match compile_graph(&text) {
            Ok(compiled) => (crate::graph::warnings_of(&compiled), false),
            Err(diagnostics) => (diagnostics, true),
        };
        for diagnostic in diagnostics {
            found.push(Diagnostic {
                severity: match diagnostic.severity {
                    ScriptSeverity::Error => Severity::Error,
                    ScriptSeverity::Warning => Severity::Warning,
                    ScriptSeverity::Info => Severity::Info,
                },
                code: diagnostic.code,
                target: target.clone(),
                message: diagnostic.message,
                action: if is_error {
                    "Abra o grafo e corrija o nó indicado na mensagem.".to_string()
                } else {
                    "Ligue o nó a um evento ou apague-o.".to_string()
                },
            });
        }
    }
    for handle in world.entities_with_table("Script").unwrap_or_default() {
        let Ok(Value::Text(file)) = world.get(handle, "Script", "file") else {
            continue;
        };
        if file.is_empty() || names.contains(file) || graphs.contains(file) {
            continue;
        }
        let id = world.persistent_id(handle).map_or(0, |id| id.0);
        found.push(Diagnostic::warning(
            "SCRIPT_MISSING",
            &format!("entidade {id}"),
            &format!(
                "o objeto aponta para o script `{file}`, que não existe em {SCRIPTS_DIRECTORY}/"
            ),
            "Crie o script com esse nome, ou aponte o objeto para outro.",
        ));
    }
    found
}

//! Scripts in the editor: the files of the project's `scripts/` folder, checked and formatted by
//! the Script Host, tried on a copy of the world, and listed in the Project Doctor.
//!
//! The editor never runs a script against the real world: "Run" simulates on a copy and reports
//! what would change, so trying a script cannot damage the scene. Saving keeps the previous version
//! in `scripts/.backup/`, because a script file is user work that the undo history does not cover.

use std::path::PathBuf;

use petunia_commands::CommandRecord;
use petunia_project::{atomic_write, read_limited, resolve_inside};
use petunia_script::{
    MAX_SCRIPT_BYTES, SCRIPT_EXTENSION, SCRIPTS_DIRECTORY, ScriptDiagnostic, api_json, compile,
    format_script, script_surface, simulate,
};
use petunia_world::{PersistentId, Value};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::EditorSession;

/// A script name is a file name: short, plain, starting with a letter.
const MAX_NAME_LENGTH: usize = 40;
const BACKUP_DIRECTORY: &str = ".backup";
/// "Run" plays this many frames of this length.
const DEFAULT_RUN_TICKS: u32 = 60;
const MAX_RUN_TICKS: u32 = 3600;
const RUN_FRAME_SECONDS: f64 = 1.0 / 60.0;

const TEMPLATE: &str = "# Script do objeto. O motor chama estas funções quando a coisa acontece.\n\
\n\
fn on_start(entity) {\n\
    log.info(\"começou\", entity)\n\
}\n\
\n\
fn on_update(entity, dt) {\n\
    # roda todo quadro; `dt` são os segundos desde o quadro anterior\n\
}\n";

fn valid_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && name.len() <= MAX_NAME_LENGTH
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn io_failure(action: &str, error: &dyn std::fmt::Display) -> Failure {
    Failure::new(
        "SCRIPT_IO",
        &format!("{action}: {error}"),
        "Confira se a pasta do projeto pode ser escrita e se há espaço no disco.",
    )
}

pub(crate) fn diagnostics_json(diagnostics: &[ScriptDiagnostic]) -> Json {
    Json::Array(
        diagnostics
            .iter()
            .map(|d| {
                json!({
                    "severity": d.severity.name(),
                    "code": d.code,
                    "message": d.message,
                    "line": d.line,
                    "column": d.column,
                    "end_line": d.end_line,
                    "end_column": d.end_column,
                    "notes": d.notes,
                })
            })
            .collect(),
    )
}

impl EditorSession {
    fn script_name(name: &str) -> Result<&str, Failure> {
        if valid_name(name) {
            Ok(name)
        } else {
            Err(Failure::new(
                "SCRIPT_BAD_NAME",
                &format!("`{name}` não serve de nome de script"),
                "Use letras, números, `_` e `-`, começando por uma letra (até 40 caracteres).",
            ))
        }
    }

    fn scripts_directory(&self) -> Result<PathBuf, Failure> {
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        resolve_inside(&project.root, SCRIPTS_DIRECTORY)
            .map_err(|error| io_failure("a pasta de scripts", &error))
    }

    fn script_path(&self, name: &str) -> Result<PathBuf, Failure> {
        let name = Self::script_name(name)?;
        Ok(self
            .scripts_directory()?
            .join(format!("{name}.{SCRIPT_EXTENSION}")))
    }

    pub(crate) fn read_script_file(&self, name: &str) -> Result<String, Failure> {
        let path = self.script_path(name)?;
        let bytes = read_limited(&path, MAX_SCRIPT_BYTES as u64 + 1).map_err(|error| {
            Failure::new(
                "SCRIPT_NOT_FOUND",
                &format!("não consegui ler o script `{name}`: {error}"),
                "Veja a lista de scripts, ou crie um novo.",
            )
        })?;
        String::from_utf8(bytes).map_err(|_| {
            Failure::new(
                "SCRIPT_NOT_TEXT",
                &format!("o script `{name}` não é texto UTF-8"),
                "Abra o arquivo num editor externo e salve como UTF-8.",
            )
        })
    }

    /// Names of the node graphs (`<name>.graph.json`) on disk, sorted; a name that also has a
    /// `.aipo` file is left out (the doctor reports the clash).
    pub(crate) fn graph_names(&self) -> Vec<String> {
        let Ok(directory) = self.scripts_directory() else {
            return Vec::new();
        };
        let scripts = self.script_names();
        petunia_script::graph_names(&directory)
            .into_iter()
            .filter(|name| valid_name(name) && !scripts.contains(name))
            .collect()
    }

    pub(crate) fn read_graph_file(&self, name: &str) -> Result<String, Failure> {
        let name = Self::script_name(name)?;
        let path = self
            .scripts_directory()?
            .join(format!("{name}{}", petunia_script::GRAPH_SUFFIX));
        let bytes =
            read_limited(&path, petunia_script::graph_max_bytes() as u64 + 1).map_err(|error| {
                Failure::new(
                    "GRAPH_NOT_FOUND",
                    &format!("não consegui ler o grafo `{name}`: {error}"),
                    "Veja a lista de scripts, ou crie um novo.",
                )
            })?;
        String::from_utf8(bytes).map_err(|_| {
            Failure::new(
                "GRAPH_NOT_TEXT",
                &format!("o grafo `{name}` não é texto UTF-8"),
                "Abra o arquivo num editor externo e salve como UTF-8.",
            )
        })
    }

    /// Names of the scripts on disk, sorted.
    pub(crate) fn script_names(&self) -> Vec<String> {
        let Ok(directory) = self.scripts_directory() else {
            return Vec::new();
        };
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
            .filter(|name| valid_name(name))
            .collect();
        names.sort();
        names
    }

    /// How many objects have this script attached.
    fn attached_count(&self, name: &str) -> usize {
        let Ok(world) = self.world() else {
            return 0;
        };
        world
            .entities_with_table("Script")
            .unwrap_or_default()
            .into_iter()
            .filter(|handle| {
                matches!(world.get(*handle, "Script", "file"), Ok(Value::Text(file)) if file == name)
            })
            .count()
    }

    pub fn script_list(&self) -> Result<Json, Failure> {
        self.world()?;
        let surface = script_surface();
        let scripts: Vec<Json> = self
            .script_names()
            .into_iter()
            .map(|name| {
                let text = self.read_script_file(&name).unwrap_or_default();
                let errors = compile(&format!("{name}.{SCRIPT_EXTENSION}"), &text, &surface)
                    .diagnostics
                    .iter()
                    .filter(|d| d.is_error())
                    .count();
                json!({
                    "name": name,
                    "bytes": text.len(),
                    "attached": self.attached_count(&name),
                    "errors": errors,
                })
            })
            .collect();
        Ok(json!({ "scripts": scripts, "max_bytes": MAX_SCRIPT_BYTES }))
    }

    pub fn script_read(&self, name: &str) -> Result<Json, Failure> {
        let text = self.read_script_file(name)?;
        Ok(json!({ "name": name, "text": text }))
    }

    /// Creates a script from the template. An existing file is never overwritten.
    pub fn script_new(&mut self, name: &str) -> Result<Json, Failure> {
        let path = self.script_path(name)?;
        if path.exists() {
            return Err(Failure::new(
                "SCRIPT_EXISTS",
                &format!("já existe um script chamado `{name}`"),
                "Escolha outro nome, ou abra o que já existe.",
            ));
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_failure("criar a pasta", &error))?;
        }
        atomic_write(&path, TEMPLATE.as_bytes())
            .map_err(|error| io_failure("criar o script", &error))?;
        self.refresh_script_diagnostics();
        Ok(json!({ "name": name, "text": TEMPLATE }))
    }

    /// Saves a script. The version it replaces goes to `scripts/.backup/` first.
    pub fn script_write(&mut self, name: &str, text: &str) -> Result<Json, Failure> {
        if text.len() > MAX_SCRIPT_BYTES {
            return Err(Failure::new(
                "SCRIPT_TOO_BIG",
                &format!(
                    "o script tem {} bytes; o máximo é {MAX_SCRIPT_BYTES}",
                    text.len()
                ),
                "Divida em dois scripts.",
            ));
        }
        let path = self.script_path(name)?;
        if path.exists() {
            let backup_directory = self.scripts_directory()?.join(BACKUP_DIRECTORY);
            if std::fs::create_dir_all(&backup_directory).is_ok() {
                // A failed backup must not lose the save: the old text is still on disk.
                let _ = std::fs::copy(
                    &path,
                    backup_directory.join(format!("{name}.{SCRIPT_EXTENSION}")),
                );
            }
        } else if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_failure("criar a pasta", &error))?;
        }
        atomic_write(&path, text.as_bytes())
            .map_err(|error| io_failure("gravar o script", &error))?;
        self.record("script", &format!("gravar {name}"), true, "");
        self.refresh_script_diagnostics();
        let compiled = compile(
            &format!("{name}.{SCRIPT_EXTENSION}"),
            text,
            &script_surface(),
        );
        Ok(
            json!({ "name": name, "bytes": text.len(), "diagnostics": diagnostics_json(&compiled.diagnostics) }),
        )
    }

    /// Checks text that may not be saved yet.
    pub fn script_check(&self, name: &str, text: &str) -> Result<Json, Failure> {
        let name = Self::script_name(name)?;
        let compiled = compile(
            &format!("{name}.{SCRIPT_EXTENSION}"),
            text,
            &script_surface(),
        );
        Ok(json!({ "diagnostics": diagnostics_json(&compiled.diagnostics) }))
    }

    pub fn script_format(&self, name: &str, text: &str) -> Result<Json, Failure> {
        let name = Self::script_name(name)?;
        let formatted =
            format_script(&format!("{name}.{SCRIPT_EXTENSION}"), text).map_err(|reason| {
                Failure::new(
                    "SCRIPT_FORMAT",
                    &format!("não deu para formatar: {reason}"),
                    "Corrija os erros de sintaxe primeiro; só texto que compila é formatado.",
                )
            })?;
        Ok(json!({ "text": formatted }))
    }

    /// Plays `ticks` frames of the script on a copy of the world and reports what happened. The
    /// editor's unsaved text, when given, stands in for the file.
    pub fn script_run(
        &self,
        name: &str,
        text: Option<&str>,
        ticks: Option<u32>,
    ) -> Result<Json, Failure> {
        let name = Self::script_name(name)?;
        let world = self.world()?;
        let mut scripts: Vec<(String, String)> = Vec::new();
        for other in self.script_names() {
            if other != name
                && let Ok(source) = self.read_script_file(&other)
            {
                scripts.push((other, source));
            }
        }
        let source = match text {
            Some(text) => text.to_string(),
            None => self.read_script_file(name)?,
        };
        scripts.push((name.to_string(), source));
        let ticks = ticks.unwrap_or(DEFAULT_RUN_TICKS).clamp(1, MAX_RUN_TICKS);
        match simulate(world, &scripts, name, ticks, RUN_FRAME_SECONDS) {
            Ok(report) => Ok(json!({
                "ok": report.faults.is_empty(),
                "ticks": report.ticks,
                "commands": report.commands,
                "created": report.created,
                "removed": report.removed,
                "changed": report.changed,
                "log": report.log.iter().map(|(level, tick, text)| json!({ "level": level, "tick": tick, "text": text })).collect::<Vec<_>>(),
                "faults": report.faults.iter().map(|fault| json!({
                    "entity": fault.entity,
                    "script": fault.script,
                    "function": fault.function,
                    "code": fault.code,
                    "message": fault.message,
                })).collect::<Vec<_>>(),
                "diagnostics": [],
            })),
            Err(diagnostics) => Ok(json!({
                "ok": false,
                "ticks": 0,
                "commands": 0,
                "created": 0,
                "removed": 0,
                "changed": 0,
                "log": [],
                "faults": [],
                "diagnostics": diagnostics_json(&diagnostics),
            })),
        }
    }

    pub fn script_api(&self) -> Json {
        api_json()
    }

    /// Attaches a script to the selected object (or changes which one it runs).
    pub fn script_attach(&mut self, name: &str) -> Result<Json, Failure> {
        let name = Self::script_name(name)?.to_string();
        let entity = self.selection.ok_or_else(|| {
            Failure::new(
                "NO_SELECTION",
                "nenhum objeto selecionado",
                "Clique num objeto primeiro.",
            )
        })?;
        if !self.script_names().contains(&name) {
            return Err(Failure::new(
                "SCRIPT_NOT_FOUND",
                &format!("o script `{name}` não existe"),
                "Crie o script antes de ligá-lo a um objeto.",
            ));
        }
        let has_row = {
            let world = self.world()?;
            world
                .handle_of(PersistentId(entity))
                .ok()
                .and_then(|handle| world.snapshot(handle).ok())
                .is_some_and(|snapshot| snapshot.rows.contains_key("Script"))
        };
        let record = if has_row {
            CommandRecord::new(
                "world.set",
                json!({ "entity": entity, "table": "Script", "column": "file", "value": name }),
            )
        } else {
            CommandRecord::new(
                "row.create",
                json!({ "entity": entity, "table": "Script", "values": { "file": name, "enabled": true } }),
            )
        };
        self.execute(&record)?;
        self.refresh_script_diagnostics();
        Ok(json!({ "entity": entity, "script": name }))
    }

    /// Recomputes the script findings of the Project Doctor (see `petunia_script::project_diagnostics`).
    pub(crate) fn refresh_script_diagnostics(&mut self) {
        let found = match (&self.project, self.world()) {
            (Some(project), Ok(world)) => petunia_script::project_diagnostics(&project.root, world),
            _ => Vec::new(),
        };
        self.script_diagnostics = found;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value as Json, json};

    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    fn opened(label: &str) -> (EditorSession, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": label }),
        );
        (session, root)
    }

    #[test]
    fn create_edit_check_format_attach_and_run_a_script() {
        let (mut session, root) = opened("scripts");
        assert_eq!(
            call(&mut session, json!({ "op": "script.list" }))["scripts"],
            json!([])
        );

        let created = call(&mut session, json!({ "op": "script.new", "name": "porta" }));
        assert!(
            created["text"]
                .as_str()
                .is_some_and(|text| text.contains("on_update"))
        );
        let again = session.handle(&json!({ "op": "script.new", "name": "porta" }));
        assert_eq!(again["error"]["code"], "SCRIPT_EXISTS", "never overwrites");
        let bad_name = session.handle(&json!({ "op": "script.new", "name": "../fuga" }));
        assert_eq!(bad_name["error"]["code"], "SCRIPT_BAD_NAME", "{bad_name}");

        // A script with a mistake is reported with a line, and does not block anything.
        let broken = "fn on_update(entity, dt) {\n    world.fnd(\"Brush\")\n}\n";
        let checked = call(
            &mut session,
            json!({ "op": "script.check", "name": "porta", "text": broken }),
        );
        assert_eq!(checked["diagnostics"][0]["line"], 2, "{checked}");
        let saved = call(
            &mut session,
            json!({ "op": "script.write", "name": "porta", "text": broken }),
        );
        assert_eq!(saved["diagnostics"][0]["severity"], "error");
        let report = call(&mut session, json!({ "op": "diagnostics.get" }));
        assert!(
            report.to_string().contains("scripts/porta.aipo:2"),
            "the Doctor lists it: {report}"
        );
        let snapshot = call(&mut session, json!({ "op": "editor.snapshot" }));
        assert!(snapshot["diagnostics"]["errors"].as_u64() >= Some(1));

        // Saving again keeps the previous text in .backup.
        let good = "fn on_update(entity, dt) {\n    log.info(\"oi\", entity)\n}\n";
        call(
            &mut session,
            json!({ "op": "script.write", "name": "porta", "text": good }),
        );
        let backup =
            std::fs::read_to_string(root.join("scripts/.backup/porta.aipo")).expect("backup");
        assert_eq!(backup, broken);
        let after = call(&mut session, json!({ "op": "diagnostics.get" }));
        assert!(
            !after.to_string().contains("scripts/porta.aipo"),
            "fixed: {after}"
        );

        // Formatting needs text that parses.
        let messy = "fn on_update(entity,dt){\nlog.info(\"oi\")\n}\n";
        assert!(
            call(
                &mut session,
                json!({ "op": "script.format", "name": "porta", "text": messy })
            )["text"]
                .is_string()
        );
        let refused =
            session.handle(&json!({ "op": "script.format", "name": "porta", "text": "fn (" }));
        assert_eq!(refused["error"]["code"], "SCRIPT_FORMAT");

        // Attaching needs a selected object; a missing script is listed by the Doctor.
        let none = session.handle(&json!({ "op": "script.attach", "name": "porta" }));
        assert_eq!(none["ok"], false);
        call(&mut session, json!({ "op": "selection.set", "entity": 9 }));
        call(
            &mut session,
            json!({ "op": "script.attach", "name": "porta" }),
        );
        assert_eq!(
            call(&mut session, json!({ "op": "script.list" }))["scripts"][0]["attached"],
            1
        );
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.set",
                    "payload": { "entity": 9, "table": "Script", "column": "file", "value": "fantasma" } }),
        );
        call(
            &mut session,
            json!({ "op": "script.write", "name": "porta", "text": good }),
        );
        let missing = call(&mut session, json!({ "op": "diagnostics.get" }));
        assert!(missing.to_string().contains("SCRIPT_MISSING"), "{missing}");

        // Run: on a copy. Nothing changes in the real project.
        call(
            &mut session,
            json!({ "op": "script.attach", "name": "porta" }),
        );
        let before = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"].clone();
        let run = call(
            &mut session,
            json!({ "op": "script.run", "name": "porta", "ticks": 5 }),
        );
        assert_eq!(run["ok"], true, "{run}");
        assert_eq!(run["log"].as_array().map(Vec::len), Some(5), "{run}");
        assert_eq!(
            call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"],
            before
        );
        let broke = call(
            &mut session,
            json!({ "op": "script.run", "name": "porta", "text": "fn (" }),
        );
        assert_eq!(broke["ok"], false);
        assert!(
            broke["diagnostics"]
                .as_array()
                .is_some_and(|d| !d.is_empty())
        );

        let api = call(&mut session, json!({ "op": "script.api" }));
        assert!(api["functions"].as_array().is_some_and(|f| f.len() > 10));
        let _ = std::fs::remove_dir_all(&root);
    }
}

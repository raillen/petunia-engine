//! Scripts in the editor: the files of the project's `scripts/` folder, checked and formatted by
//! the Script Host, tried on a copy of the world, and listed in the Project Doctor.
//!
//! The editor never runs a script against the real world: "Run" simulates on a copy and reports
//! what would change, so trying a script cannot damage the scene. Saving keeps the previous version
//! in `scripts/.backup/`, because a script file is user work that the undo history does not cover.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

pub(crate) fn valid_name(name: &str) -> bool {
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

    /// The objects that name each script or graph in their `Script` row. A legacy row on the
    /// object has `owner` 0; a child attachment points at the object with `owner`.
    fn attached_scripts(&self) -> BTreeMap<String, Vec<Json>> {
        let Ok(world) = self.world() else {
            return BTreeMap::new();
        };
        let mut attached: BTreeMap<String, Vec<Json>> = BTreeMap::new();
        for handle in world.entities_with_table("Script").unwrap_or_default() {
            let Ok(id) = world.persistent_id(handle) else {
                continue;
            };
            let Ok(Value::Text(file)) = world.get(handle, "Script", "file") else {
                continue;
            };
            if file.is_empty() {
                continue;
            }
            let owner = script_owner_of(world, id.0).unwrap_or(id.0);
            let objects = attached.entry(file.clone()).or_default();
            if !objects.iter().any(|object| object["id"] == owner) {
                objects.push(json!({
                    "id": owner,
                    "name": crate::session::display_name(world, owner),
                }));
            }
        }
        attached
    }

    /// `(attachment entity, file, order)` of the Script rows that belong to `owner`.
    fn attachment_rows(&self, owner: u64) -> Vec<(u64, String, i64)> {
        let Ok(world) = self.world() else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for handle in world.entities_with_table("Script").unwrap_or_default() {
            let Ok(id) = world.persistent_id(handle) else {
                continue;
            };
            let actual_owner = script_owner_of(world, id.0).unwrap_or(id.0);
            if actual_owner != owner {
                continue;
            }
            let Ok(Value::Text(file)) = world.get(handle, "Script", "file") else {
                continue;
            };
            let order = match world.get(handle, "Script", "order") {
                Ok(Value::Int(order)) => *order,
                _ => 0,
            };
            rows.push((id.0, file.clone(), order));
        }
        rows
    }

    pub fn script_list(&self) -> Result<Json, Failure> {
        self.world()?;
        let surface = script_surface();
        let names = self.script_names();
        let graphs = self.graph_names();
        let attached = self.attached_scripts();
        let scripts: Vec<Json> = names
            .iter()
            .map(|name| {
                let text = self.read_script_file(name).unwrap_or_default();
                let errors = compile(&format!("{name}.{SCRIPT_EXTENSION}"), &text, &surface)
                    .diagnostics
                    .iter()
                    .filter(|d| d.is_error())
                    .count();
                json!({
                    "name": name,
                    "bytes": text.len(),
                    "attached": attached.get(name).map_or(0, Vec::len),
                    "errors": errors,
                })
            })
            .collect();
        let known: BTreeSet<&String> = names.iter().chain(graphs.iter()).collect();
        let missing: Vec<Json> = attached
            .iter()
            .filter(|(name, _)| !known.contains(name))
            .map(|(name, entities)| json!({ "name": name, "attached": entities }))
            .collect();
        Ok(json!({ "scripts": scripts, "missing": missing, "max_bytes": MAX_SCRIPT_BYTES }))
    }

    /// `programming.list`: every script and graph of the project, with the objects that run it.
    /// A name in a `Script` row that has no file is listed as `missing`, so the workspace can show
    /// what the person asked for before the file exists.
    pub fn programming_list(&self) -> Result<Json, Failure> {
        self.world()?;
        let attached = self.attached_scripts();
        let mut items = Vec::new();
        let mut known = BTreeSet::new();
        for name in self.script_names() {
            known.insert(name.clone());
            items.push(json!({
                "name": name,
                "kind": "script",
                "attached": attached.get(&name).cloned().unwrap_or_default(),
            }));
        }
        for name in self.graph_names() {
            known.insert(name.clone());
            items.push(json!({
                "name": name,
                "kind": "graph",
                "attached": attached.get(&name).cloned().unwrap_or_default(),
            }));
        }
        for (name, entities) in attached {
            if !known.contains(&name) {
                items.push(json!({
                    "name": name,
                    "kind": "missing",
                    "attached": entities,
                }));
            }
        }
        Ok(json!({ "items": items, "selected": self.selection }))
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

    /// Attaches a script to the selected object. The first script keeps the legacy row on the
    /// object; a second one converts that row into a child attachment and adds another child, so
    /// both run. Attaching the same file again is a no-op.
    pub fn script_attach(&mut self, name: &str) -> Result<Json, Failure> {
        let name = Self::script_name(name)?.to_string();
        let selected = self.selection.ok_or_else(|| {
            Failure::new(
                "NO_SELECTION",
                "nenhum objeto selecionado",
                "Clique num objeto primeiro.",
            )
        })?;
        let entity = {
            let world = self.world()?;
            crate::colliders::owner_of(world, selected)
                .or_else(|| script_owner_of(world, selected))
                .unwrap_or(selected)
        };
        if !self.script_names().contains(&name) && !self.graph_names().contains(&name) {
            return Err(Failure::new(
                "SCRIPT_NOT_FOUND",
                &format!("o script `{name}` não existe"),
                "Crie o script antes de ligá-lo a um objeto.",
            ));
        }
        let rows = self.attachment_rows(entity);
        if let Some((attachment, _, _)) = rows.iter().find(|(_, file, _)| file == &name) {
            return Ok(json!({ "entity": entity, "script": name, "attachment": attachment }));
        }
        let next_order = rows
            .iter()
            .map(|(_, _, order)| *order)
            .max()
            .map_or(0, |highest| highest.saturating_add(1));
        let legacy = {
            let world = self.world()?;
            let handle = world
                .handle_of(PersistentId(entity))
                .map_err(Failure::from)?;
            if world.has_row(handle, "Script") {
                let file = match world.get(handle, "Script", "file") {
                    Ok(Value::Text(file)) => file.clone(),
                    _ => String::new(),
                };
                let enabled = matches!(
                    world.get(handle, "Script", "enabled"),
                    Ok(Value::Bool(true))
                );
                (!file.is_empty()).then_some((file, enabled))
            } else {
                None
            }
        };
        let result = if let Some((legacy_file, legacy_enabled)) = legacy {
            let records = vec![
                CommandRecord::new(
                    "world.create",
                    json!({ "rows": { "Script": {
                        "owner": entity, "file": legacy_file, "enabled": legacy_enabled, "order": 0
                    } } }),
                ),
                CommandRecord::new(
                    "world.create",
                    json!({ "rows": { "Script": {
                        "owner": entity, "file": name, "enabled": true, "order": next_order
                    } } }),
                ),
                CommandRecord::new("row.remove", json!({ "entity": entity, "table": "Script" })),
            ];
            let outcome = self.execute(&CommandRecord::new(
                petunia_commands::BATCH_COMMAND,
                json!({ "records": records }),
            ))?;
            outcome
                .get(1)
                .and_then(|created| created.get("entity"))
                .and_then(Json::as_u64)
        } else if rows.is_empty() {
            self.execute(&CommandRecord::new(
                "row.create",
                json!({ "entity": entity, "table": "Script",
                        "values": { "file": name, "enabled": true } }),
            ))?;
            Some(entity)
        } else {
            let outcome = self.execute(&CommandRecord::new(
                "world.create",
                json!({ "rows": { "Script": {
                    "owner": entity, "file": name, "enabled": true, "order": next_order
                } } }),
            ))?;
            outcome.get("entity").and_then(Json::as_u64)
        };
        self.refresh_script_diagnostics();
        Ok(json!({ "entity": entity, "script": name, "attachment": result }))
    }

    /// `world.remove` records for the child script attachments of `owner` (deleting an object
    /// deletes them; the legacy row travels with the object itself).
    pub(crate) fn remove_script_records(&self, owner: u64) -> Vec<CommandRecord> {
        self.attachment_rows(owner)
            .into_iter()
            .filter(|(id, _, _)| *id != owner)
            .map(|(id, _, _)| CommandRecord::new("world.remove", json!({ "entity": id })))
            .collect()
    }

    /// `world.create` records that copy the child script attachments of `owner` onto the entity
    /// created by command number `copy` of the same batch.
    pub(crate) fn copy_script_records(&self, owner: u64, copy: usize) -> Vec<CommandRecord> {
        let Ok(world) = self.world() else {
            return Vec::new();
        };
        self.attachment_rows(owner)
            .into_iter()
            .filter(|(id, _, _)| *id != owner)
            .filter_map(|(id, _, _)| {
                let handle = world.handle_of(PersistentId(id)).ok()?;
                let values = world.row(handle, "Script").ok()?;
                let mut row = world.row_to_json("Script", values).ok()?;
                row.insert("owner".to_string(), json!({ "$entity_of": copy }));
                Some(CommandRecord::new(
                    "world.create",
                    json!({ "rows": { "Script": row } }),
                ))
            })
            .collect()
    }

    /// `(attachment entity, owner object, file)` for every Script row naming `file`.
    fn attachment_rows_for_file(&self, file: &str) -> Vec<(u64, u64, String)> {
        let Ok(world) = self.world() else {
            return Vec::new();
        };
        let mut rows = Vec::new();
        for handle in world.entities_with_table("Script").unwrap_or_default() {
            let Ok(id) = world.persistent_id(handle) else {
                continue;
            };
            let Ok(Value::Text(name)) = world.get(handle, "Script", "file") else {
                continue;
            };
            if name != file {
                continue;
            }
            let owner = script_owner_of(world, id.0).unwrap_or(id.0);
            rows.push((id.0, owner, name.clone()));
        }
        rows
    }

    /// Which files an existing script or graph name owns, and whether the request asked for a
    /// specific kind. `kind` comes from the list the person clicked; it disambiguates a name that
    /// has both `.aipo` and `.graph.json` (the Doctor reports that clash).
    fn name_files(
        &self,
        name: &str,
        kind: Option<&str>,
    ) -> Result<(&'static str, std::path::PathBuf, Option<std::path::PathBuf>), Failure> {
        let script = self.script_path(name)?;
        let graph = self
            .scripts_directory()?
            .join(format!("{name}{}", petunia_script::GRAPH_SUFFIX));
        let layout = graph.with_file_name(format!("{name}.layout.json"));
        let has_script = script.is_file();
        let has_graph = graph.is_file();
        match kind {
            Some("script") if has_script => Ok(("script", script, None)),
            Some("graph") if has_graph => Ok(("graph", graph, layout.is_file().then_some(layout))),
            Some("script") | Some("graph") => Err(Failure::new(
                "SCRIPT_NOT_FOUND",
                &format!("não existe {} chamado `{name}`", kind.unwrap_or("arquivo")),
                "Atualize a lista e tente de novo.",
            )),
            Some(other) => Err(Failure::bad_request(&format!(
                "`kind` é `script` ou `graph` (veio `{other}`)"
            ))),
            None if has_script && has_graph => Err(Failure::new(
                "SCRIPT_NAME_CLASH",
                &format!("`{name}` existe como script e como grafo"),
                "Apague ou renomeie um dos dois num editor de texto.",
            )),
            None if has_script => Ok(("script", script, None)),
            None if has_graph => Ok(("graph", graph, layout.is_file().then_some(layout))),
            None => Err(Failure::new(
                "SCRIPT_NOT_FOUND",
                &format!("não existe script ou grafo chamado `{name}`"),
                "Atualize a lista e tente de novo.",
            )),
        }
    }

    fn target_is_free(&self, name: &str) -> Result<(), Failure> {
        let directory = self.scripts_directory()?;
        let candidates = [
            directory.join(format!("{name}.{SCRIPT_EXTENSION}")),
            directory.join(format!("{name}{}", petunia_script::GRAPH_SUFFIX)),
            directory.join(format!("{name}.layout.json")),
        ];
        if candidates.iter().any(|path| path.exists()) {
            return Err(Failure::new(
                "SCRIPT_EXISTS",
                &format!("já existe um arquivo chamado `{name}`"),
                "Escolha outro nome.",
            ));
        }
        Ok(())
    }

    /// `script.rename`: renames the file(s) and every `Script` row that points at the old name.
    pub fn script_rename(
        &mut self,
        old: &str,
        new: &str,
        kind: Option<&str>,
    ) -> Result<Json, Failure> {
        let old = Self::script_name(old)?.to_string();
        let new = Self::script_name(new)?.to_string();
        if old == new {
            return Ok(json!({ "name": new, "kind": kind.unwrap_or("") }));
        }
        self.target_is_free(&new)?;
        let (kind, path, extra) = self.name_files(&old, kind)?;
        let new_path = if kind == "script" {
            path.with_file_name(format!("{new}.{SCRIPT_EXTENSION}"))
        } else {
            path.with_file_name(format!("{new}{}", petunia_script::GRAPH_SUFFIX))
        };
        let new_extra = extra
            .as_ref()
            .map(|_| path.with_file_name(format!("{new}.layout.json")));
        std::fs::rename(&path, &new_path)
            .map_err(|error| io_failure("renomear o script", &error))?;
        if let (Some(extra_path), Some(new_extra_path)) = (extra.as_ref(), new_extra.as_ref())
            && let Err(error) = std::fs::rename(extra_path, new_extra_path)
        {
            let _ = std::fs::rename(&new_path, &path);
            return Err(io_failure("renomear o layout", &error));
        }

        let attachments = self.attachment_rows_for_file(&old);
        if !attachments.is_empty() {
            let records: Vec<CommandRecord> = attachments
                .iter()
                .map(|(entity, _, _)| {
                    CommandRecord::new(
                        "world.set",
                        json!({ "entity": entity, "table": "Script", "column": "file", "value": new }),
                    )
                })
                .collect();
            if let Err(failure) = self.execute(&CommandRecord::new(
                petunia_commands::BATCH_COMMAND,
                json!({ "records": records }),
            )) {
                let _ = std::fs::rename(&new_path, &path);
                if let (Some(extra_path), Some(new_extra_path)) =
                    (extra.as_ref(), new_extra.as_ref())
                {
                    let _ = std::fs::rename(new_extra_path, extra_path);
                }
                return Err(failure);
            }
        }
        if kind == "graph"
            && let Some(graph) = self.graph.as_mut()
            && graph.name == old
        {
            graph.name = new.clone();
        }
        self.refresh_script_diagnostics();
        Ok(json!({ "name": new, "kind": kind }))
    }

    /// `script.delete`: moves the file(s) to `scripts/.backup/` (never destroys them) and removes
    /// the attachments. Without `force`, a script still attached to an object is refused.
    pub fn script_delete(
        &mut self,
        name: &str,
        kind: Option<&str>,
        force: bool,
    ) -> Result<Json, Failure> {
        let name = Self::script_name(name)?.to_string();
        let (kind, path, extra) = self.name_files(&name, kind)?;
        let attachments = self.attachment_rows_for_file(&name);
        if !attachments.is_empty() && !force {
            return Err(Failure::new(
                "SCRIPT_IN_USE",
                &format!("`{name}` está ligado a {} objeto(s)", attachments.len()),
                "Desanexe-o primeiro, ou confirme a exclusão para desligar de todos.",
            ));
        }
        let backup = self.scripts_directory()?.join(BACKUP_DIRECTORY);
        std::fs::create_dir_all(&backup)
            .map_err(|error| io_failure("criar a pasta de backup", &error))?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let extension = if kind == "graph" {
            "graph.json".to_string()
        } else {
            SCRIPT_EXTENSION.to_string()
        };
        let backup_path = backup.join(format!("{name}.{stamp}.{extension}"));
        std::fs::rename(&path, &backup_path)
            .map_err(|error| io_failure("mover o script para o backup", &error))?;
        if let Some(extra_path) = extra.as_ref() {
            let extra_backup = backup.join(format!("{name}.{stamp}.layout.json"));
            if let Err(error) = std::fs::rename(extra_path, &extra_backup) {
                let _ = std::fs::rename(&backup_path, &path);
                return Err(io_failure("mover o layout para o backup", &error));
            }
        }
        if !attachments.is_empty() {
            let records: Vec<CommandRecord> = attachments
                .iter()
                .map(|(entity, owner, _)| {
                    if entity == owner {
                        CommandRecord::new(
                            "row.remove",
                            json!({ "entity": entity, "table": "Script" }),
                        )
                    } else {
                        CommandRecord::new("world.remove", json!({ "entity": entity }))
                    }
                })
                .collect();
            if let Err(failure) = self.execute(&CommandRecord::new(
                petunia_commands::BATCH_COMMAND,
                json!({ "records": records }),
            )) {
                let _ = std::fs::rename(&backup_path, &path);
                return Err(failure);
            }
        }
        if kind == "graph" && self.graph.as_ref().is_some_and(|graph| graph.name == name) {
            self.graph = None;
        }
        self.refresh_script_diagnostics();
        Ok(json!({ "deleted": name, "kind": kind, "backup": backup_path }))
    }

    /// Recomputes the script findings of the Project Doctor (see `petunia_script::project_diagnostics`).
    pub(crate) fn refresh_script_diagnostics(&mut self) {
        let found = match (&self.project, self.world()) {
            (Some(project), Ok(world)) => petunia_script::project_diagnostics(
                &project.root,
                world,
                &project.manifest.programming_method,
            ),
            _ => Vec::new(),
        };
        self.script_diagnostics = found;
    }
}

/// The object a Script attachment belongs to, for a child attachment (`owner > 0`).
pub(crate) fn script_owner_of(world: &petunia_world::World, id: u64) -> Option<u64> {
    let handle = world.handle_of(PersistentId(id)).ok()?;
    match world.get(handle, "Script", "owner") {
        Ok(Value::Int(owner)) if *owner > 0 => Some(*owner as u64),
        _ => None,
    }
}

/// The file named by a Script attachment (object or child entity).
pub(crate) fn script_file_of(world: &petunia_world::World, id: u64) -> Option<String> {
    let handle = world.handle_of(PersistentId(id)).ok()?;
    match world.get(handle, "Script", "file") {
        Ok(Value::Text(file)) if !file.is_empty() => Some(file.clone()),
        _ => None,
    }
}

/// How many Script rows are child attachments (not the legacy row on the object).
pub(crate) fn script_attachment_count(world: &petunia_world::World) -> usize {
    world
        .entities_with_table("Script")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|handle| world.persistent_id(handle).ok())
        .filter(|id| script_owner_of(world, id.0).is_some())
        .count()
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
    fn programming_lists_attached_graphs_scripts_and_missing_files() {
        let (mut session, root) = opened("programming");

        call(&mut session, json!({ "op": "selection.set", "entity": 9 }));
        call(&mut session, json!({ "op": "graph.new", "name": "gira" }));
        call(
            &mut session,
            json!({ "op": "script.attach", "name": "gira" }),
        );

        let listed = call(&mut session, json!({ "op": "programming.list" }));
        let items = listed["items"].as_array().expect("items");
        let graph = items
            .iter()
            .find(|item| item["name"] == "gira")
            .expect("graph item");
        assert_eq!(graph["kind"], "graph");
        assert_eq!(graph["attached"][0]["id"], 9);

        // A Script row naming a file that does not exist is surfaced, not hidden.
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.set",
                    "payload": { "entity": 9, "table": "Script", "column": "file", "value": "fantasma" } }),
        );
        let listed = call(&mut session, json!({ "op": "programming.list" }));
        let items = listed["items"].as_array().expect("items");
        let missing = items
            .iter()
            .find(|item| item["name"] == "fantasma")
            .expect("missing item");
        assert_eq!(missing["kind"], "missing");
        assert_eq!(missing["attached"][0]["id"], 9);

        // The code workspace lists the same missing name so it can be created there.
        let listed = call(&mut session, json!({ "op": "script.list" }));
        assert_eq!(listed["missing"][0]["name"], "fantasma", "{listed}");

        let _ = std::fs::remove_dir_all(&root);
    }

    fn child_scripts_of(session: &mut EditorSession, owner: u64) -> usize {
        let snapshot = call(session, json!({ "op": "editor.snapshot" }));
        snapshot["entities"]
            .as_array()
            .expect("entities")
            .iter()
            .filter(|entity| entity["owner"] == owner)
            .filter(|entity| {
                entity["tables"]
                    .as_array()
                    .is_some_and(|tables| tables.iter().any(|table| table == "Script"))
            })
            .count()
    }

    #[test]
    fn scripts_and_graphs_can_be_renamed_and_deleted_without_losing_the_backup() {
        let (mut session, root) = opened("rename_delete");
        call(&mut session, json!({ "op": "script.new", "name": "a" }));
        call(&mut session, json!({ "op": "script.new", "name": "b" }));
        call(&mut session, json!({ "op": "selection.set", "entity": 9 }));
        call(&mut session, json!({ "op": "script.attach", "name": "a" }));

        let renamed = call(
            &mut session,
            json!({ "op": "script.rename", "name": "a", "new": "c", "kind": "script" }),
        );
        assert_eq!(renamed["name"], "c", "{renamed}");
        assert!(root.join("scripts/c.aipo").is_file());
        assert!(!root.join("scripts/a.aipo").exists());
        let listed = call(&mut session, json!({ "op": "programming.list" }));
        let item = listed["items"]
            .as_array()
            .expect("items")
            .iter()
            .find(|item| item["name"] == "c")
            .expect("renamed item");
        assert_eq!(item["attached"][0]["id"], 9, "{item}");

        // A new name that already exists is refused and nothing changes.
        let refused = session
            .handle(&json!({ "op": "script.rename", "name": "c", "new": "b", "kind": "script" }));
        assert_eq!(refused["error"]["code"], "SCRIPT_EXISTS");
        assert!(root.join("scripts/c.aipo").is_file());

        // Without force, an attached script is not deleted; with force it goes to .backup.
        let refused =
            session.handle(&json!({ "op": "script.delete", "name": "c", "kind": "script" }));
        assert_eq!(refused["error"]["code"], "SCRIPT_IN_USE");
        let deleted = call(
            &mut session,
            json!({ "op": "script.delete", "name": "c", "kind": "script", "force": true }),
        );
        assert!(
            deleted["backup"]
                .as_str()
                .is_some_and(|path| path.contains(".backup/"))
        );
        assert!(!root.join("scripts/c.aipo").exists());
        let listed = call(&mut session, json!({ "op": "programming.list" }));
        assert!(
            listed["items"]
                .as_array()
                .expect("items")
                .iter()
                .all(|item| item["name"] != "c"),
            "{listed}"
        );

        // Graphs go through the same path, including the layout file.
        call(&mut session, json!({ "op": "graph.new", "name": "g" }));
        call(&mut session, json!({ "op": "script.attach", "name": "g" }));
        let renamed = call(
            &mut session,
            json!({ "op": "script.rename", "name": "g", "new": "h", "kind": "graph" }),
        );
        assert_eq!(renamed["kind"], "graph");
        assert!(root.join("scripts/h.graph.json").is_file());
        assert_eq!(
            call(&mut session, json!({ "op": "graph.view" }))["name"],
            "h"
        );
        call(
            &mut session,
            json!({ "op": "script.delete", "name": "h", "kind": "graph", "force": true }),
        );
        assert!(!root.join("scripts/h.graph.json").exists());
        assert_eq!(
            call(&mut session, json!({ "op": "graph.list" }))["graphs"],
            json!([])
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn several_scripts_on_one_object_run_in_order_and_travel_with_it() {
        let (mut session, root) = opened("multi_scripts");
        for name in ["a", "b"] {
            call(
                &mut session,
                json!({ "op": "script.write", "name": name,
                        "text": format!("fn on_update(entity, dt) {{ log.info(\"{name}\") }}\n") }),
            );
        }
        call(&mut session, json!({ "op": "selection.set", "entity": 9 }));
        call(&mut session, json!({ "op": "script.attach", "name": "a" }));
        let second = call(&mut session, json!({ "op": "script.attach", "name": "b" }));
        assert!(
            second["attachment"].as_u64().is_some_and(|id| id != 9),
            "the second script is a child attachment: {second}"
        );
        assert_eq!(child_scripts_of(&mut session, 9), 2);
        let listed = call(&mut session, json!({ "op": "programming.list" }));
        for name in ["a", "b"] {
            let item = listed["items"]
                .as_array()
                .expect("items")
                .iter()
                .find(|item| item["name"] == name)
                .expect("script item");
            assert_eq!(item["attached"][0]["id"], 9, "{item}");
        }

        // Both survive save/reopen as child attachments.
        call(&mut session, json!({ "op": "project.save" }));
        let mut reopened = EditorSession::new().expect("session");
        call(
            &mut reopened,
            json!({ "op": "project.open", "path": root.display().to_string() }),
        );
        call(&mut reopened, json!({ "op": "selection.set", "entity": 9 }));
        assert_eq!(child_scripts_of(&mut reopened, 9), 2);
        call(&mut reopened, json!({ "op": "play.start" }));
        let tick = call(
            &mut reopened,
            json!({ "op": "play.tick", "seconds": 1.0 / 60.0 }),
        );
        let texts: Vec<&str> = tick["log"]
            .as_array()
            .expect("log")
            .iter()
            .filter_map(|line| line["text"].as_str())
            .collect();
        assert!(texts.contains(&"a") && texts.contains(&"b"), "{texts:?}");
        call(&mut reopened, json!({ "op": "play.stop" }));

        // Both run in the same frame; the host calls each attachment.
        let started = call(&mut session, json!({ "op": "play.start" }));
        assert!(started["problems"].as_array().expect("list").is_empty());
        let tick = call(
            &mut session,
            json!({ "op": "play.tick", "seconds": 1.0 / 60.0 }),
        );
        let texts: Vec<&str> = tick["log"]
            .as_array()
            .expect("log")
            .iter()
            .filter_map(|line| line["text"].as_str())
            .collect();
        assert!(texts.contains(&"a") && texts.contains(&"b"), "{texts:?}");
        call(&mut session, json!({ "op": "play.stop" }));

        // Duplicating the object copies the attachments; deleting it removes them.
        call(&mut session, json!({ "op": "selection.set", "entity": 9 }));
        let copy = call(&mut session, json!({ "op": "edit.duplicate" }))["entity"]
            .as_u64()
            .expect("copy");
        assert_eq!(child_scripts_of(&mut session, copy), 2);
        call(&mut session, json!({ "op": "selection.set", "entity": 9 }));
        call(&mut session, json!({ "op": "edit.delete" }));
        assert_eq!(child_scripts_of(&mut session, 9), 0);
        assert_eq!(child_scripts_of(&mut session, copy), 2);
        let _ = std::fs::remove_dir_all(&root);
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

        // Run: on a copy. Nothing changes in the real project. Put the object back on the
        // existing script row (the missing name was only a Doctor test).
        call(
            &mut session,
            json!({ "op": "command.execute", "command": "world.set",
                    "payload": { "entity": 9, "table": "Script", "column": "file", "value": "porta" } }),
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

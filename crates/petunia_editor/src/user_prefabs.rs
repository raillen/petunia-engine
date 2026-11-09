//! Prefabs of the project itself: a saved mold of a selection that can be placed again later.
//!
//! The built-in prefabs (Porta, Item, Ponto de nascimento, Personagem) live in `prefabs.rs`. This
//! module saves **your own**: it takes the selected object with its colliders, script attachments
//! and sculpt strokes, writes them to `prefabs/<name>.prefab.json` (text, so it diffs) and places
//! a copy in front of the camera.
//!
//! Positions in the file are **relative to the object that was saved**, so a prefab spawns where
//! the camera points, not back where it was made. Saving the same name again bumps the `revision`
//! in the file; a copy in the scene is **never rewritten** by that (changing objects the person
//! already moved needs its own decision), so the revision is a record for now.

use petunia_commands::{BATCH_COMMAND, CommandRecord};
use petunia_math::Vec3;
use petunia_project::{atomic_write, read_limited, resolve_inside};
use petunia_world::PersistentId;
use serde_json::{Map, Value as Json, json};

use crate::colliders::colliders_of;
use crate::failure::Failure;
use crate::session::{EditorSession, display_name, vector_json};

pub const PREFABS_DIRECTORY: &str = "prefabs";
const PREFAB_SUFFIX: &str = ".prefab.json";
/// Prefix that tells a project prefab from a built-in one in `prefab.spawn`.
pub const PREFAB_PREFIX: &str = "project:";
/// Version of the file shape; a file from another version is refused, not guessed.
const FORMAT_VERSION: u64 = 1;
/// Ceilings: a prefab file is external data and is validated like any other.
const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ATTACHMENTS: usize = 128;
const MAX_NAME_CHARS: usize = 60;
/// A saved prefab goes this far in front of the camera when placed.
const PLACE_DISTANCE: f32 = 5.0;

fn prefab_failure(code: &str, message: &str, action: &str) -> Failure {
    Failure::new(code, message, action)
}

fn file_failure(detail: &str) -> Failure {
    prefab_failure(
        "PREFAB_FILE",
        detail,
        "Apague o arquivo e salve o prefab de novo.",
    )
}

/// A file name that is safe inside `prefabs/`: letters, digits, `-`, `_`, `.`; never starting
/// with a dot, never empty.
fn clean_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_start_matches('.').trim();
    if trimmed.is_empty() {
        "prefab".to_string()
    } else {
        trimmed.chars().take(MAX_NAME_CHARS).collect()
    }
}

/// One object of the mold: its rows plus the entities that point at it (colliders, strokes,
/// script attachments).
struct Part {
    rows: Map<String, Json>,
    /// `(table, reference column, row)`: the reference is rewired to the copy when it is placed.
    attachments: Vec<(String, String, Map<String, Json>)>,
}

struct Prefab {
    name: String,
    revision: u64,
    parts: Vec<Part>,
}

fn read_prefab(path: &std::path::Path) -> Result<Prefab, Failure> {
    let bytes = read_limited(path, MAX_FILE_BYTES).map_err(|error| {
        prefab_failure(
            "PREFAB_IO",
            &format!("ler o prefab `{}`: {error}", path.display()),
            "Confira se o arquivo existe e não está corrompido.",
        )
    })?;
    let text = String::from_utf8(bytes).map_err(|_| file_failure("o prefab não é texto UTF-8"))?;
    let document: Json = serde_json::from_str(&text)
        .map_err(|error| file_failure(&format!("o prefab não é JSON válido: {error}")))?;
    if document.get("format").and_then(Json::as_u64) != Some(FORMAT_VERSION) {
        return Err(prefab_failure(
            "PREFAB_FILE",
            "o prefab foi escrito por outra versão do editor",
            "Salve o prefab de novo nesta versão.",
        ));
    }
    let name = document
        .get("name")
        .and_then(Json::as_str)
        .unwrap_or_default()
        .to_string();
    let revision = document.get("revision").and_then(Json::as_u64).unwrap_or(0);
    let Some(objects) = document.get("objects").and_then(Json::as_array) else {
        return Err(file_failure("o prefab não tem a lista `objects`"));
    };
    let mut parts = Vec::with_capacity(objects.len());
    for object in objects {
        let Some(rows) = object.get("rows").and_then(Json::as_object) else {
            return Err(file_failure("um objeto do prefab não tem `rows`"));
        };
        let attachments = object
            .get("attachments")
            .and_then(Json::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        if attachments.len() > MAX_ATTACHMENTS {
            return Err(file_failure("um objeto do prefab tem anexos demais"));
        }
        let mut pairs = Vec::with_capacity(attachments.len());
        for attachment in attachments {
            let (Some(table), Some(reference), Some(row)) = (
                attachment.get("table").and_then(Json::as_str),
                attachment.get("reference").and_then(Json::as_str),
                attachment.get("row").and_then(Json::as_object),
            ) else {
                return Err(file_failure(
                    "um anexo do prefab não tem `table`, `reference` e `row`",
                ));
            };
            if table.is_empty() || reference.is_empty() || table.len() > 64 || reference.len() > 64
            {
                return Err(file_failure("nome de tabela ou coluna inválido no prefab"));
            }
            pairs.push((table.to_string(), reference.to_string(), row.clone()));
        }
        parts.push(Part {
            rows: rows.clone(),
            attachments: pairs,
        });
    }
    Ok(Prefab {
        name,
        revision,
        parts,
    })
}

fn position_of(value: &Json) -> Option<Vec3> {
    Some(Vec3::new(
        value.get("x")?.as_f64()? as f32,
        value.get("y")?.as_f64()? as f32,
        value.get("z")?.as_f64()? as f32,
    ))
}

/// `(table, reference column)` of the things that hang on an object.
fn attachment_of(
    world: &petunia_world::World,
    entity: u64,
) -> Option<(&'static str, &'static str)> {
    let handle = world.handle_of(PersistentId(entity)).ok()?;
    for (table, reference) in [
        ("Collider", "owner"),
        ("SculptStroke", "volume"),
        ("Script", "owner"),
    ] {
        if world.row(handle, table).is_ok() {
            return Some((table, reference));
        }
    }
    None
}

fn attachment_row(
    world: &petunia_world::World,
    entity: u64,
    table: &str,
    reference: &str,
) -> Option<(String, String, Map<String, Json>)> {
    let handle = world.handle_of(PersistentId(entity)).ok()?;
    let values = world.row(handle, table).ok()?;
    let mut row = world.row_to_json(table, values).ok()?;
    row.insert(reference.to_string(), Json::Null);
    Some((table.to_string(), reference.to_string(), row))
}

/// The prefab files in the project folder, sorted by name.
fn prefab_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(root.join(PREFABS_DIRECTORY)) else {
        return Vec::new();
    };
    let mut paths: Vec<std::path::PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(PREFAB_SUFFIX) && !name.starts_with('.'))
        })
        // Symbolic links are not followed: a link could point outside the project.
        .filter(|path| std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file()))
        .collect();
    paths.sort();
    paths
}

fn prefab_name_of(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.trim_end_matches(PREFAB_SUFFIX).to_string())
        .unwrap_or_default()
}

impl EditorSession {
    fn prefab_path(&self, name: &str) -> Result<(String, std::path::PathBuf), Failure> {
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        let title = clean_name(name);
        let relative = format!("{PREFABS_DIRECTORY}/{title}{PREFAB_SUFFIX}");
        let path = resolve_inside(&project.root, &relative).map_err(|error| {
            prefab_failure(
                "PREFAB_IO",
                &format!("o caminho do prefab: {error}"),
                "Use um nome sem barras.",
            )
        })?;
        Ok((relative, path))
    }

    /// `prefab.save`: writes the selected object (with its colliders, script attachments and
    /// sculpt strokes) as a prefab of the project. Saving the same name again bumps the revision.
    pub fn prefab_save(&mut self, name: &str) -> Result<Json, Failure> {
        let selected = self.selection.ok_or_else(|| {
            prefab_failure(
                "PREFAB_NO_SELECTION",
                "nada está selecionado",
                "Clique no objeto que vira prefab, no viewport ou na Hierarquia.",
            )
        })?;
        let (relative, path) = self.prefab_path(name)?;
        let world = self.world()?;
        // A collider or a script attachment is part of an object, not a prefab of its own.
        let id = crate::colliders::owner_of(world, selected)
            .or_else(|| {
                crate::scripts::script_owner_of(world, selected).filter(|owner| *owner != selected)
            })
            .unwrap_or(selected);
        let handle = world.handle_of(PersistentId(id)).map_err(Failure::from)?;
        let snapshot = world.snapshot(handle).map_err(Failure::from)?;

        let mut rows = Map::new();
        for (table, values) in &snapshot.rows {
            rows.insert(
                table.clone(),
                Json::Object(world.row_to_json(table, values).map_err(Failure::from)?),
            );
        }
        // The saved position becomes the origin: the file says where the object sits relative to
        // the mold, so it lands where the camera points instead of back where it was saved.
        if let Some(transform) = rows.get_mut("Transform")
            && let Some(position) = transform.get_mut("position")
        {
            *position = vector_json(Vec3::ZERO);
        }

        let mut found: Vec<u64> = colliders_of(world, id);
        found.extend(self.strokes_of(id));
        found.extend(
            world
                .entities_with_table("Script")
                .unwrap_or_default()
                .into_iter()
                .filter_map(|handle| world.persistent_id(handle).ok().map(|id| id.0))
                .filter(|attachment| *attachment != id)
                .filter(|attachment| {
                    crate::scripts::script_owner_of(world, *attachment) == Some(id)
                }),
        );
        let mut attachments: Vec<Json> = Vec::new();
        for attachment in found {
            let Some((table, reference)) = attachment_of(world, attachment) else {
                continue;
            };
            if let Some((table, reference, row)) =
                attachment_row(world, attachment, table, reference)
            {
                attachments.push(json!({
                    "table": table,
                    "reference": reference,
                    "row": Json::Object(row),
                }));
            }
        }
        if attachments.len() > MAX_ATTACHMENTS {
            return Err(prefab_failure(
                "PREFAB_TOO_BIG",
                &format!("o objeto tem mais de {MAX_ATTACHMENTS} anexos"),
                "Separe o objeto em partes menores antes de salvar o prefab.",
            ));
        }

        let title = clean_name(name);
        // Saving over an existing prefab keeps its history: the revision goes up.
        let revision = read_prefab(&path)
            .map(|previous| previous.revision + 1)
            .unwrap_or(1);
        let document = json!({
            "format": FORMAT_VERSION,
            "name": title,
            "revision": revision,
            "objects": [{ "rows": Json::Object(rows), "attachments": attachments }],
        });
        let text = serde_json::to_string_pretty(&document).map_err(|error| {
            prefab_failure(
                "PREFAB_FILE",
                &format!("escrever o prefab: {error}"),
                "Isto é um defeito do Petunia, não do projeto.",
            )
        })?;
        atomic_write(&path, text.as_bytes()).map_err(|error| {
            prefab_failure(
                "PREFAB_IO",
                &format!("gravar `{relative}`: {error}"),
                "Confira se há espaço no disco e se a pasta do projeto pode ser escrita.",
            )
        })?;
        let saved = display_name(world, id);
        self.record(
            "prefab.save",
            &format!("prefab `{title}` salvo (revisão {revision})"),
            true,
            &saved,
        );
        Ok(json!({
            "prefab": format!("{PREFAB_PREFIX}{title}"),
            "name": title,
            "revision": revision,
            "path": relative,
        }))
    }

    /// The prefabs of the project, as the editor lists them.
    pub fn project_prefabs(&self) -> Vec<Json> {
        let Ok(project) = self.project.as_ref().ok_or_else(Failure::no_project) else {
            return Vec::new();
        };
        prefab_files(&project.root)
            .into_iter()
            .filter_map(|path| {
                let name = prefab_name_of(&path);
                let prefab = read_prefab(&path).ok()?;
                Some(json!({
                    "id": format!("{PREFAB_PREFIX}{name}"),
                    "kind": "project",
                    "title": name,
                    "summary": format!(
                        "Prefab do projeto (revisão {}): recria os objetos salvos com colisões, scripts e traços.",
                        prefab.revision
                    ),
                    "revision": prefab.revision,
                    "objects": prefab.parts.len(),
                    "attachments": prefab.parts.iter().map(|part| part.attachments.len()).sum::<usize>(),
                    "path": format!("{PREFABS_DIRECTORY}/{name}{PREFAB_SUFFIX}"),
                }))
            })
            .collect()
    }

    /// `prefab.spawn_project`: places a saved prefab in front of the camera, on the current floor,
    /// as **one** undo step.
    pub(crate) fn prefab_spawn_project(&mut self, name: &str) -> Result<Json, Failure> {
        let (relative, path) = self.prefab_path(name)?;
        if !path.is_file() {
            return Err(prefab_failure(
                "PREFAB_UNKNOWN",
                &format!("não existe o prefab `{}`", clean_name(name)),
                "Salve um com Criar > Salvar seleção como prefab….",
            ));
        }
        let prefab = read_prefab(&path)?;
        if prefab.parts.is_empty() {
            return Err(file_failure("o prefab não tem objetos"));
        }

        let camera = self.camera.camera;
        let forward = camera.forward();
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalized_or_zero();
        let flat = if flat.length_squared() == 0.0 {
            Vec3::new(0.0, 0.0, -1.0)
        } else {
            flat
        };
        let target = camera.position + flat * PLACE_DISTANCE;
        let at = Vec3::new(target.x, self.floor_elevation(), target.z);

        let mut records: Vec<CommandRecord> = Vec::new();
        for (index, part) in prefab.parts.iter().enumerate() {
            let mut rows = part.rows.clone();
            if let Some(transform) = rows.get_mut("Transform")
                && let Some(position) = transform.get_mut("position")
            {
                *position = vector_json(position_of(position).unwrap_or(Vec3::ZERO) + at);
            }
            records.push(CommandRecord::new("world.create", json!({ "rows": rows })));
            for (table, reference, row) in &part.attachments {
                let mut row = row.clone();
                row.insert(reference.clone(), json!({ "$entity_of": index }));
                records.push(CommandRecord::new(
                    "world.create",
                    json!({ "rows": { table.clone(): Json::Object(row) } }),
                ));
            }
        }
        let result = self.execute(&CommandRecord::new(
            BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        let entity = result
            .get(0)
            .and_then(|first| first.get("entity"))
            .and_then(Json::as_u64);
        self.selection = entity;
        self.refresh_script_diagnostics();
        let name = prefab.name;
        let revision = prefab.revision;
        self.record(
            "prefab.spawn",
            &format!("prefab `{name}` (revisão {revision}) criado"),
            true,
            &relative,
        );
        Ok(json!({
            "entity": entity,
            "graphs_written": Vec::<String>::new(),
            "graphs_kept": Vec::<String>::new(),
            "prefab": name,
            "revision": revision,
        }))
    }
}

#[cfg(test)]
mod tests {
    // Test code: a failed expectation should stop the test with a message.
    use petunia_math::Vec3;
    use serde_json::{Value as Json, json};

    use super::PREFAB_PREFIX;
    use crate::session::EditorSession;

    fn call(session: &mut EditorSession, request: Json) -> Json {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], true, "{request} -> {reply}");
        reply["result"].clone()
    }

    fn scratch(label: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "petunia_editor_test_{label}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        root
    }

    fn position(session: &mut EditorSession, entity: u64) -> Vec3 {
        session
            .frame()
            .expect("frame")
            .items
            .into_iter()
            .find(|item| item.entity == entity)
            .map(|item| item.transform.position)
            .expect("the object is drawn")
    }

    #[test]
    fn a_saved_prefab_comes_back_with_its_colliders_and_one_undo() {
        let root = scratch("prefab_save");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "p" }),
        );
        let before = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"]
            .as_u64()
            .expect("count");

        // A box with a collider of its own.
        let made = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let source = made["entity"].as_u64().expect("box id");
        call(
            &mut session,
            json!({ "op": "collider.add", "shape": "box" }),
        );

        let saved = call(
            &mut session,
            json!({ "op": "prefab.save", "name": "Minha Caixa" }),
        );
        // A name with a space becomes a file-safe name; the point is that it round-trips.
        assert_eq!(saved["name"], "Minha_Caixa", "{saved}");
        assert_eq!(saved["revision"], 1);
        let file = root.join("prefabs/Minha_Caixa.prefab.json");
        assert!(file.is_file(), "the prefab file exists");
        let text = std::fs::read_to_string(&file).expect("read");
        assert!(text.contains("\"format\": 1"), "{text}");
        assert!(text.contains("Collider"), "the collider was saved: {text}");
        let saved_count = text.matches("\"table\"").count();
        assert_eq!(saved_count, 1, "one attachment (the collider): {text}");

        // The list has the built-ins plus ours.
        let listed = call(&mut session, json!({ "op": "prefab.list" }));
        let prefabs = listed["prefabs"].as_array().expect("list");
        assert_eq!(prefabs.len(), 5, "{listed}");
        assert!(
            prefabs
                .iter()
                .any(|prefab| prefab["id"] == json!(format!("{PREFAB_PREFIX}Minha_Caixa"))),
            "{listed}"
        );

        // Placing it recreates the object and its collider, in front of the camera, in one step.
        let placed = call(
            &mut session,
            json!({ "op": "prefab.spawn", "id": format!("{PREFAB_PREFIX}Minha_Caixa") }),
        );
        let copy = placed["entity"].as_u64().expect("copy id");
        assert_ne!(copy, source);
        let count = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"]
            .as_u64()
            .expect("count");
        assert_eq!(
            count,
            before + 4,
            "the box, its collider, and the copy with its collider"
        );
        let described = call(
            &mut session,
            json!({ "op": "entity.describe", "entity": copy }),
        );
        assert_eq!(described["name"], "Caixa", "{described}");

        call(&mut session, json!({ "op": "edit.undo" }));
        let undone = call(&mut session, json!({ "op": "editor.snapshot" }))["entity_count"]
            .as_u64()
            .expect("count");
        assert_eq!(
            undone,
            before + 2,
            "one undo took the copy and its collider"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_prefab_lands_in_front_of_the_camera_not_where_it_was_saved() {
        let root = scratch("prefab_place");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "p" }),
        );
        let made = call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let source = made["entity"].as_u64().expect("box id");
        call(
            &mut session,
            json!({ "op": "prefab.save", "name": "caixa" }),
        );
        // Move the object away from where it was saved: the copy must land in front of the
        // camera, not back at the object's own place.
        let saved_at = position(&mut session, source);
        let moved = saved_at + Vec3::new(60.0, 0.0, 60.0);
        call(
            &mut session,
            json!({ "op": "gesture.preview", "entity": source, "table": "Transform",
                    "column": "position",
                    "value": { "x": moved.x, "y": moved.y, "z": moved.z } }),
        );
        call(&mut session, json!({ "op": "gesture.commit" }));
        let placed = call(
            &mut session,
            json!({ "op": "prefab.spawn", "id": format!("{PREFAB_PREFIX}caixa") }),
        );
        let copy = placed["entity"].as_u64().expect("copy id");
        let landed = position(&mut session, copy);
        assert!(
            (landed.x - moved.x).abs() > 30.0 && (landed.z - moved.z).abs() > 30.0,
            "saved at {moved:?}, placed at {landed:?}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn saving_twice_bumps_the_revision_and_bad_files_are_refused() {
        let root = scratch("prefab_revision");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "p" }),
        );
        call(
            &mut session,
            json!({ "op": "create.brush", "shape": "box" }),
        );
        let first = call(
            &mut session,
            json!({ "op": "prefab.save", "name": "torre" }),
        );
        assert_eq!(first["revision"], 1);
        let second = call(
            &mut session,
            json!({ "op": "prefab.save", "name": "torre" }),
        );
        assert_eq!(second["revision"], 2, "{second}");

        // A broken file is refused with an explanation, and nothing is spawned.
        std::fs::write(root.join("prefabs/torre.prefab.json"), "{ isso não é json").expect("write");
        let reply = session.handle(&json!({
            "op": "prefab.spawn",
            "id": format!("{PREFAB_PREFIX}torre"),
        }));
        assert_eq!(reply["ok"], false, "{reply}");
        assert_eq!(reply["error"]["code"], "PREFAB_FILE", "{reply}");

        // An unknown name says so instead of failing silently.
        let missing = session.handle(&json!({
            "op": "prefab.spawn",
            "id": format!("{PREFAB_PREFIX}nada"),
        }));
        assert_eq!(missing["error"]["code"], "PREFAB_UNKNOWN", "{missing}");

        // Saving with nothing selected explains what to do.
        call(&mut session, json!({ "op": "selection.set" }));
        let empty = session.handle(&json!({ "op": "prefab.save", "name": "vazio" }));
        assert_eq!(empty["error"]["code"], "PREFAB_NO_SELECTION", "{empty}");
        let _ = std::fs::remove_dir_all(&root);
    }
}

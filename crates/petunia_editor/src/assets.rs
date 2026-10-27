//! The project's `assets/` folder and importing 3D models (GLB, glTF) into the scene (DR-29).
//!
//! An imported model becomes **editable meshes** (the `Mesh` table of DR-22): one object per node
//! of the file that has geometry, with its place, rotation, scale and base color. They select,
//! move, get colliders and take the mesh tools like anything else. The file is copied into
//! `assets/`, so the project keeps what it was made from. Importing is one undo step.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use petunia_commands::{BATCH_COMMAND, CommandRecord};
use petunia_import::{ImportError, MAX_FILE_BYTES, import_gltf};
use petunia_math::{Transform, Vec3};
use petunia_project::{atomic_write, read_limited};
use petunia_render::model_matrix;
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::session::{EditorSession, vector_json};

pub const ASSETS_DIRECTORY: &str = "assets";
const MODEL_EXTENSIONS: [&str; 2] = ["glb", "gltf"];
/// How deep the folder is read and how many files are listed.
const MAX_DEPTH: usize = 4;
const MAX_LISTED: usize = 500;
/// A new model goes this far in front of the camera.
const PLACE_DISTANCE: f32 = 6.0;

fn failure(code: &str, message: &str, action: &str) -> Failure {
    Failure::new(code, message, action)
}

fn io_failure(action: &str, error: &dyn std::fmt::Display) -> Failure {
    failure(
        "ASSET_IO",
        &format!("{action}: {error}"),
        "Confira se o arquivo existe e se a pasta do projeto pode ser escrita.",
    )
}

fn is_model(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            MODEL_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
}

/// A file name that is safe inside `assets/`: letters, digits, `-`, `_`, `.`; never starting with a dot.
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
    let trimmed = cleaned.trim_start_matches('.');
    if trimmed.is_empty() {
        "modelo".to_string()
    } else {
        trimmed.chars().take(80).collect()
    }
}

fn collect(directory: &Path, root: &Path, depth: usize, out: &mut Vec<Json>) {
    if depth > MAX_DEPTH || out.len() >= MAX_LISTED {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    for path in paths {
        if out.len() >= MAX_LISTED {
            return;
        }
        // Symbolic links are not followed: a link could point outside the project.
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            collect(&path, root, depth + 1, out);
        } else if metadata.is_file() && is_model(&path) {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            out.push(json!({
                "name": path.file_name().and_then(|name| name.to_str()).unwrap_or(""),
                "path": relative,
                "kind": "model",
                "bytes": metadata.len(),
            }));
        }
    }
}

impl EditorSession {
    fn assets_root(&self) -> Result<PathBuf, Failure> {
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        Ok(project.root.join(ASSETS_DIRECTORY))
    }

    /// `asset.list`: the models in the project's `assets/` folder (paths relative to the project).
    pub fn asset_list(&self) -> Result<Json, Failure> {
        let root = self.assets_root()?;
        let mut files = Vec::new();
        collect(&root, &root, 0, &mut files);
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        // Paths are shown relative to the project root.
        for file in &mut files {
            if let Some(path) = file.get("path").and_then(Json::as_str) {
                file["path"] = json!(format!("{ASSETS_DIRECTORY}/{path}"));
            }
        }
        let _ = project;
        Ok(json!({ "assets": files, "folder": ASSETS_DIRECTORY, "max_bytes": MAX_FILE_BYTES }))
    }

    /// `asset.import`: puts a model into the scene. `path` is a file inside `assets/` (relative to
    /// the project, like the list gives) or any file on disk, which is copied into `assets/`.
    pub fn asset_import(&mut self, path: &str) -> Result<Json, Failure> {
        let assets = self.assets_root()?;
        let given = PathBuf::from(path);
        let project_root = self
            .project
            .as_ref()
            .ok_or_else(Failure::no_project)?
            .root
            .clone();
        let source = if given.is_absolute() {
            given
        } else {
            project_root.join(&given)
        };
        let source = std::fs::canonicalize(&source)
            .map_err(|error| io_failure(&format!("abrir `{path}`"), &error))?;
        if !source.is_file() || !is_model(&source) {
            return Err(failure(
                "ASSET_UNSUPPORTED",
                "só arquivos .glb e .gltf podem ser importados",
                "Escolha um modelo exportado como glTF binário (.glb).",
            ));
        }
        let folder = source.parent().map(Path::to_path_buf).unwrap_or_default();
        let bytes = read_limited(&source, MAX_FILE_BYTES as u64)
            .map_err(|error| io_failure("ler o modelo", &error))?;

        // External files (the `.bin` of a .gltf) are read only from the model's own folder.
        let used: RefCell<Vec<(String, Vec<u8>)>> = RefCell::new(Vec::new());
        let resolve = |name: &str| -> Option<Vec<u8>> {
            let file = Path::new(name);
            let plain = file.components().count() == 1
                && file
                    .components()
                    .all(|component| matches!(component, std::path::Component::Normal(_)));
            if !plain {
                return None;
            }
            let data = read_limited(&folder.join(file), MAX_FILE_BYTES as u64).ok()?;
            used.borrow_mut().push((name.to_string(), data.clone()));
            Some(data)
        };
        let model = import_gltf(&bytes, &resolve).map_err(|error| match error {
            ImportError::Invalid(message) => failure(
                "ASSET_INVALID",
                &format!("o arquivo não é um modelo válido: {message}"),
                "Exporte de novo do programa de modelagem.",
            ),
            ImportError::Refused(message) => failure(
                "ASSET_REFUSED",
                &message,
                "Simplifique o modelo ou divida-o em partes.",
            ),
        })?;

        // Keep what the project was made from: a copy inside assets/ (unless it already is there).
        let inside = std::fs::canonicalize(&assets).is_ok_and(|assets| source.starts_with(assets));
        let stored = if inside {
            source
                .strip_prefix(std::fs::canonicalize(&assets).unwrap_or(assets.clone()))
                .map(Path::to_path_buf)
                .unwrap_or_default()
        } else {
            let stem = clean_name(
                source
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("modelo.glb"),
            );
            let mut target = assets.join(&stem);
            let mut number = 2;
            while target.exists() {
                let base = Path::new(&stem)
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or("modelo");
                let extension = Path::new(&stem)
                    .extension()
                    .and_then(|name| name.to_str())
                    .unwrap_or("glb");
                target = assets.join(format!("{base}-{number}.{extension}"));
                number += 1;
            }
            atomic_write(&target, &bytes)
                .map_err(|error| io_failure("copiar o modelo para assets/", &error))?;
            for (name, data) in used.borrow().iter() {
                let neighbour = assets.join(clean_name(name));
                if !neighbour.exists() {
                    atomic_write(&neighbour, data)
                        .map_err(|error| io_failure("copiar um arquivo do modelo", &error))?;
                }
            }
            target
                .strip_prefix(&assets)
                .map(Path::to_path_buf)
                .unwrap_or_default()
        };

        // Where it goes: the model's own bottom-center lands in front of the camera, on the floor.
        let mut low = Vec3::splat(f32::MAX);
        let mut high = Vec3::splat(f32::MIN);
        for node in &model.nodes {
            let transform = Transform {
                position: node.position,
                rotation_degrees: node.rotation_degrees,
                scale: node.scale,
            };
            let matrix = model_matrix(&transform);
            let (box_low, box_high) = node.mesh.bounds().unwrap_or((Vec3::ZERO, Vec3::ZERO));
            for corner in 0..8 {
                let local = Vec3::new(
                    if corner & 1 == 0 {
                        box_low.x
                    } else {
                        box_high.x
                    },
                    if corner & 2 == 0 {
                        box_low.y
                    } else {
                        box_high.y
                    },
                    if corner & 4 == 0 {
                        box_low.z
                    } else {
                        box_high.z
                    },
                );
                let entry = |index: usize| matrix.get(index).copied().unwrap_or(0.0);
                let world = Vec3::new(
                    entry(0) * local.x + entry(4) * local.y + entry(8) * local.z + entry(12),
                    entry(1) * local.x + entry(5) * local.y + entry(9) * local.z + entry(13),
                    entry(2) * local.x + entry(6) * local.y + entry(10) * local.z + entry(14),
                );
                low = Vec3::new(low.x.min(world.x), low.y.min(world.y), low.z.min(world.z));
                high = Vec3::new(
                    high.x.max(world.x),
                    high.y.max(world.y),
                    high.z.max(world.z),
                );
            }
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
        let offset = Vec3::new(
            target.x - (low.x + high.x) * 0.5,
            self.floor_elevation() - low.y,
            target.z - (low.z + high.z) * 0.5,
        );

        let mut records: Vec<CommandRecord> = model
            .nodes
            .iter()
            .map(|node| {
                CommandRecord::new(
                    "world.create",
                    json!({ "rows": {
                        "Name": { "text": node.name },
                        "Transform": {
                            "position": vector_json(node.position + offset),
                            "rotation_degrees": vector_json(node.rotation_degrees),
                            "scale": vector_json(node.scale)
                        },
                        "Mesh": {
                            "geometry": node.mesh.to_text_json(),
                            "color": { "r": node.color.r, "g": node.color.g, "b": node.color.b, "a": node.color.a }
                        }
                    } }),
                )
            })
            .collect();
        let count = records.len();
        let record = match records.len() {
            0 => {
                return Err(failure(
                    "ASSET_EMPTY",
                    "o modelo não tem nenhuma malha para importar",
                    "Confira se o arquivo tem objetos com geometria (câmeras e luzes não entram).",
                ));
            }
            1 => records.remove(0),
            _ => CommandRecord::new(BATCH_COMMAND, json!({ "records": records })),
        };
        let result = self.execute(&record)?;
        let first = if count == 1 {
            result.get("entity")
        } else {
            result.get(0).and_then(|first| first.get("entity"))
        };
        let first = first.and_then(Json::as_u64);
        self.selection = first;
        let names: Vec<&str> = model.nodes.iter().map(|node| node.name.as_str()).collect();
        Ok(json!({
            "imported": count,
            "names": names,
            "asset": format!("{ASSETS_DIRECTORY}/{}", stored.display()),
            "warnings": model.warnings,
            "entity": first,
        }))
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

    fn entity_count(session: &EditorSession) -> usize {
        session.snapshot()["entities"]
            .as_array()
            .map_or(0, Vec::len)
    }

    fn call_error(session: &mut EditorSession, request: Json) -> String {
        let reply = session.handle(&request);
        assert_eq!(reply["ok"], false, "{request} -> {reply}");
        reply["error"]["code"].as_str().unwrap_or("").to_string()
    }

    /// A GLB with two named nodes sharing one triangle mesh.
    fn two_triangles_glb() -> Vec<u8> {
        let points: [[f32; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mut bin = Vec::new();
        for point in &points {
            for value in point {
                bin.extend_from_slice(&value.to_le_bytes());
            }
        }
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0,1]}}],
"nodes":[{{"name":"Esquerda","mesh":0}},{{"name":"Direita","mesh":0,"translation":[3.0,0.0,0.0]}}],
"meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}}}}]}}],
"buffers":[{{"byteLength":{len}}}],
"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{len}}}],
"accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}}]}}"#,
            len = bin.len()
        );
        let mut json_bytes = json.into_bytes();
        while !json_bytes.len().is_multiple_of(4) {
            json_bytes.push(b' ');
        }
        let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_bytes);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
        out
    }

    #[test]
    fn a_model_is_copied_into_assets_imported_as_meshes_and_undone_in_one_step() {
        let base =
            std::env::temp_dir().join(format!("petunia_editor_test_assets_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("projeto");
        std::fs::create_dir_all(&base).expect("folder");
        let outside = base.join("fora.glb");
        std::fs::write(&outside, two_triangles_glb()).expect("write model");

        let mut session = EditorSession::new().expect("session");
        assert_eq!(
            call_error(&mut session, json!({ "op": "asset.list" })),
            "EDITOR_NO_PROJECT"
        );
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "modelos" }),
        );
        assert_eq!(
            call(&mut session, json!({ "op": "asset.list" }))["assets"],
            json!([])
        );

        let before = entity_count(&session);
        let result = call(
            &mut session,
            json!({ "op": "asset.import", "path": outside.display().to_string() }),
        );
        assert_eq!(result["imported"], 2);
        assert_eq!(result["names"], json!(["Esquerda", "Direita"]));
        assert_eq!(result["asset"], "assets/fora.glb");
        assert!(root.join("assets/fora.glb").is_file(), "a copy is kept");
        assert_eq!(entity_count(&session), before + 2);

        // The copy is listed, and importing it again by its project path works.
        let listed = call(&mut session, json!({ "op": "asset.list" }));
        assert_eq!(listed["assets"][0]["path"], "assets/fora.glb");
        call(
            &mut session,
            json!({ "op": "asset.import", "path": "assets/fora.glb" }),
        );
        assert_eq!(entity_count(&session), before + 4);

        // One undo takes away the whole import (both objects), not one node at a time.
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(entity_count(&session), before + 2);
        call(&mut session, json!({ "op": "edit.undo" }));
        assert_eq!(entity_count(&session), before);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn hostile_files_and_paths_are_refused_without_touching_the_scene() {
        let base = std::env::temp_dir().join(format!(
            "petunia_editor_test_assets_bad_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).expect("folder");
        let root = base.join("projeto");
        let mut session = EditorSession::new().expect("session");
        call(
            &mut session,
            json!({ "op": "project.new_sample", "path": root.display().to_string(), "name": "ruim" }),
        );
        let before = entity_count(&session);

        let garbage = base.join("lixo.glb");
        std::fs::write(&garbage, b"isto nao e um glb").expect("write");
        assert_eq!(
            call_error(
                &mut session,
                json!({ "op": "asset.import", "path": garbage.display().to_string() })
            ),
            "ASSET_INVALID"
        );
        let text = base.join("nota.txt");
        std::fs::write(&text, b"oi").expect("write");
        assert_eq!(
            call_error(
                &mut session,
                json!({ "op": "asset.import", "path": text.display().to_string() })
            ),
            "ASSET_UNSUPPORTED"
        );
        assert_eq!(
            call_error(
                &mut session,
                json!({ "op": "asset.import", "path": base.join("nao_existe.glb").display().to_string() })
            ),
            "ASSET_IO"
        );
        assert_eq!(entity_count(&session), before);
        assert!(
            !root.join("assets/lixo.glb").exists(),
            "a refused file is not copied"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}

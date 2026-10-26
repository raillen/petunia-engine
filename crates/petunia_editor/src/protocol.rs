use std::path::Path;

use petunia_commands::{CommandRecord, QueryRecord, run_query};
use serde_json::{Value as Json, json};

use crate::failure::Failure;
use crate::gizmo::Tool;
use crate::session::EditorSession;
use crate::tools::Pointer;

fn text<'a>(request: &'a Json, key: &str) -> Result<&'a str, Failure> {
    request
        .get(key)
        .and_then(Json::as_str)
        .ok_or_else(|| Failure::bad_request(&format!("falta o campo de texto `{key}`")))
}

fn number(request: &Json, key: &str) -> Result<f32, Failure> {
    request
        .get(key)
        .and_then(Json::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .ok_or_else(|| Failure::bad_request(&format!("falta o campo numérico `{key}`")))
}

fn number_or(request: &Json, key: &str, default: f32) -> f32 {
    request
        .get(key)
        .and_then(Json::as_f64)
        .map_or(default, |value| value as f32)
}

fn pointer(request: &Json) -> Result<Pointer, Failure> {
    let pointer = Pointer {
        x: number(request, "x")?,
        y: number(request, "y")?,
        width: number(request, "width")?,
        height: number(request, "height")?,
    };
    if pointer.width < 1.0 || pointer.height < 1.0 {
        return Err(Failure::bad_request(
            "`width` e `height` precisam ser pelo menos 1",
        ));
    }
    Ok(pointer)
}

fn optional_number(request: &Json, key: &str) -> Option<f32> {
    request
        .get(key)
        .and_then(Json::as_f64)
        .map(|value| value as f32)
}

fn entity(request: &Json) -> Result<u64, Failure> {
    request
        .get("entity")
        .and_then(Json::as_u64)
        .ok_or_else(|| Failure::bad_request("falta o campo `entity` (id inteiro)"))
}

impl EditorSession {
    /// The one entry point for a GUI, a test or an agent: a JSON request in, a JSON response out.
    ///
    /// Request: `{ "op": "...", ...fields }`. Response: `{ "ok": true, "result": ... }` or
    /// `{ "ok": false, "error": { "code", "message", "action" } }`.
    pub fn handle(&mut self, request: &Json) -> Json {
        match self.dispatch(request) {
            Ok(result) => json!({ "ok": true, "result": result }),
            Err(failure) => json!({ "ok": false, "error": failure.to_json() }),
        }
    }

    fn dispatch(&mut self, request: &Json) -> Result<Json, Failure> {
        let operation = text(request, "op")?;
        // While the game runs, only looking and reading are accepted: nothing edits the scene.
        if self.is_playing() {
            const ALLOWED: [&str; 12] = [
                "play.",
                "camera.",
                "viewport.",
                "editor.",
                "entity.",
                "schema.",
                "diagnostics.",
                "history.",
                "script.",
                "graph.",
                "pointer.",
                "selection.",
            ];
            if !ALLOWED.iter().any(|prefix| operation.starts_with(prefix)) {
                return self.ensure_not_playing().map(|()| Json::Null);
            }
        }
        match operation {
            "project.new_sample" => {
                let path = text(request, "path")?;
                let name = text(request, "name").unwrap_or("Meu projeto");
                self.new_sample_project(Path::new(path), name)
            }
            "project.open" => self.open_project(Path::new(text(request, "path")?)),
            "project.save" => self.save(),

            "script.list" => self.script_list(),
            "script.read" => self.script_read(text(request, "name")?),
            "script.new" => self.script_new(text(request, "name")?),
            "script.write" => self.script_write(text(request, "name")?, text(request, "text")?),
            "script.check" => self.script_check(text(request, "name")?, text(request, "text")?),
            "script.format" => self.script_format(text(request, "name")?, text(request, "text")?),
            "script.run" => self.script_run(
                text(request, "name")?,
                request.get("text").and_then(Json::as_str),
                request
                    .get("ticks")
                    .and_then(Json::as_u64)
                    .map(|ticks| ticks.min(3600) as u32),
            ),
            "script.api" => Ok(self.script_api()),
            "script.attach" => self.script_attach(text(request, "name")?),

            "graph.list" => self.graph_list(),
            "graph.new" => self.graph_new(text(request, "name")?),
            "graph.open" => self.graph_open(text(request, "name")?),
            "graph.close" => self.graph_close(),
            "graph.view" => self.graph_view(),
            "graph.add" => self.graph_add(
                text(request, "kind")?,
                Self::graph_request_optional_pin(request, "after")?,
                Self::graph_request_optional_pin(request, "into")?,
            ),
            "graph.remove" => self.graph_remove(Self::graph_request_node(request, "node")?),
            "graph.set" => self.graph_set(
                Self::graph_request_node(request, "node")?,
                text(request, "name")?,
                request
                    .get("value")
                    .filter(|value| !value.is_null())
                    .cloned(),
            ),
            "graph.connect" => self.graph_connect(
                Self::graph_request_pin(request, "from")?,
                Self::graph_request_pin(request, "to")?,
            ),
            "graph.disconnect" => self.graph_disconnect(Self::graph_request_pin(request, "pin")?),
            "graph.undo" => self.graph_undo(),
            "graph.redo" => self.graph_redo(),
            "graph.search" => self.graph_search(
                text(request, "query").unwrap_or(""),
                text(request, "want").unwrap_or("any"),
                Self::graph_request_optional_pin(request, "for")?,
            ),
            "graph.sources" => self.graph_sources(&Self::graph_request_pin(request, "for")?),
            "graph.code" => self.graph_code(),

            "editor.snapshot" => Ok(self.snapshot()),
            "entity.describe" => self.describe(entity(request)?),
            "diagnostics.get" => Ok(self.diagnostics()),
            "history.get" => Ok(self.history()),
            "schema.get" => Ok(self.schema()),

            "command.execute" => {
                let record = CommandRecord::new(
                    text(request, "command")?,
                    request.get("payload").cloned().unwrap_or(Json::Null),
                );
                self.execute(&record)
            }
            "query.run" => {
                let record = QueryRecord::new(
                    text(request, "query")?,
                    request.get("payload").cloned().unwrap_or(Json::Null),
                );
                let world = self.world_for_query()?;
                run_query(world, &record).map_err(Failure::from)
            }
            "edit.undo" => self.undo(),
            "edit.redo" => self.redo(),
            "edit.duplicate" => self.duplicate(),
            "edit.delete" => self.delete_selection(),
            "create.brush" => self.create_brush(text(request, "shape")?),
            "create.cave" => self.create_cave(request.get("seed").and_then(Json::as_u64)),
            "sculpt.begin" => {
                let invert = request
                    .get("invert")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                self.sculpt_begin(pointer(request)?, invert)
            }
            "sculpt.mode" => self.set_sculpt_mode(
                request.get("mode").and_then(Json::as_str),
                optional_number(request, "force"),
            ),
            "sculpt.generate" => {
                let seed = request
                    .get("seed")
                    .and_then(Json::as_u64)
                    .ok_or_else(|| Failure::bad_request("falta o campo `seed` (número inteiro)"))?;
                self.sculpt_generate(
                    seed,
                    request
                        .get("rooms")
                        .and_then(Json::as_u64)
                        .map(|n| n.min(24) as u32),
                    request
                        .get("worms")
                        .and_then(Json::as_u64)
                        .map(|n| n.min(12) as u32),
                    optional_number(request, "radius"),
                )
            }
            "sculpt.consolidate" => self.sculpt_consolidate(),
            "sculpt.faceted" => {
                let value = request.get("value").and_then(Json::as_bool).unwrap_or(true);
                self.sculpt_faceted(value)
            }
            "path.point" => {
                let invert = request
                    .get("invert")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                self.path_point(pointer(request)?, invert)
            }
            "path.pop" => Ok(self.path_pop()),
            "path.commit" => self.path_commit(),
            "path.cancel" => Ok(self.path_cancel()),
            "sculpt.drag" => self.sculpt_drag(pointer(request)?),
            "sculpt.end" => self.sculpt_end(),
            "sculpt.cancel" => Ok(self.sculpt_cancel()),
            "sculpt.radius" => self.set_sculpt_radius(number(request, "meters")?),

            "gesture.preview" => {
                let value = request.get("value").cloned().unwrap_or(Json::Null);
                self.gesture_preview(
                    entity(request)?,
                    text(request, "table")?,
                    text(request, "column")?,
                    &value,
                )
            }
            "gesture.commit" => self.gesture_commit(),
            "gesture.cancel" => self.gesture_cancel(),

            "tool.set" => {
                let name = text(request, "tool")?;
                let tool = Tool::parse(name).ok_or_else(|| {
                    Failure::bad_request(&format!(
                        "ferramenta `{name}` desconhecida (select, move, rotate, scale, push_pull, extrude, sculpt ou sculpt_path)"
                    ))
                })?;
                Ok(self.set_tool(tool))
            }
            "snap.set" => self.set_snap(
                request.get("enabled").and_then(Json::as_bool),
                [
                    optional_number(request, "translate"),
                    optional_number(request, "rotate_degrees"),
                    optional_number(request, "scale"),
                ],
                request.get("target").and_then(Json::as_str),
                request.get("align_to_slope").and_then(Json::as_bool),
            ),
            "pointer.hover" => Ok(self.pointer_hover(pointer(request)?)),
            "transform.set" => self.set_transform_options(
                request.get("orientation").and_then(Json::as_str),
                request.get("pivot").and_then(Json::as_str),
                request.get("edit_pivot").and_then(Json::as_bool),
            ),
            "free.begin" => {
                let name = text(request, "tool")?;
                let tool = Tool::parse(name).ok_or_else(|| {
                    Failure::bad_request(&format!("ferramenta `{name}` desconhecida"))
                })?;
                self.free_begin(tool, pointer(request)?)
            }
            "free.axis" => {
                let axis = request.get("axis").and_then(Json::as_u64).unwrap_or(3) as usize;
                self.free_axis(axis, pointer(request)?)
            }
            "free.type" => self.free_type(
                request.get("text").and_then(Json::as_str).unwrap_or(""),
                pointer(request)?,
            ),
            "face.begin" => self.face_drag_begin(pointer(request)?),
            "face.drag" => {
                let invert = request
                    .get("invert_snap")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                self.face_drag(pointer(request)?, invert)
            }
            "face.end" => self.face_drag_end(),
            "face.cancel" => self.face_drag_cancel(),
            "brush.carve" => self.carve_selection(),
            "collider.add" => {
                self.add_collider(request.get("shape").and_then(Json::as_str).unwrap_or("box"))
            }
            "collider.fit" => self.fit_collider(),
            "asset.list" => self.asset_list(),
            "asset.import" => self.asset_import(text(request, "path")?),
            "create.light" => self.create_light(
                request
                    .get("kind")
                    .and_then(Json::as_str)
                    .unwrap_or("point"),
            ),
            "play.start" => self.play_start(),
            "play.stop" => self.play_stop(),
            "play.tick" => self.play_tick(
                number_or(request, "seconds", 0.016),
                number_or(request, "forward", 0.0),
                number_or(request, "right", 0.0),
                request.get("jump").and_then(Json::as_bool).unwrap_or(false),
                (
                    number_or(request, "look_x", 0.0),
                    number_or(request, "look_y", 0.0),
                ),
            ),
            op if op.starts_with("mesh.") => self.mesh_request(op, request),
            "brush.hollow" => self.hollow_selection(),
            "floor.step" => {
                let delta = request.get("delta").and_then(Json::as_i64).unwrap_or(0);
                self.floor_step(delta.clamp(-8, 8) as i32)
            }
            "floor.height" => self.set_floor_height(number(request, "meters")?),
            "floor.above" => self.set_above_floor(text(request, "mode")?),
            "floor.select" => {
                let entity = request
                    .get("entity")
                    .and_then(Json::as_u64)
                    .ok_or_else(|| Failure::bad_request("falta o campo `entity` (número)"))?;
                self.floor_select(entity)
            }
            "viewport.outline_style" => {
                let color = match request.get("color") {
                    Some(value) => Some(crate::tools::outline_color(value)?),
                    None => None,
                };
                self.set_outline_style(color, optional_number(request, "width"))
            }
            "viewport.overlay" => {
                Ok(self.viewport_overlay(number(request, "width")?, number(request, "height")?))
            }
            "camera.view_direction" => self.camera_view_direction(petunia_math::Vec3::new(
                number(request, "x")?,
                number(request, "y")?,
                number(request, "z")?,
            )),
            "camera.yaw" => self.camera_yaw(number(request, "degrees")?),
            "gizmo.begin" => Ok(json!({ "started": self.gizmo_begin(pointer(request)?) })),
            "gizmo.drag" => {
                let invert = request
                    .get("invert_snap")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                self.gizmo_drag(pointer(request)?, invert)
            }
            "gizmo.end" => self.gizmo_end(),
            "gizmo.cancel" => self.gizmo_cancel(),
            "camera.view" => self.camera_view(text(request, "name")?),
            "camera.orbit" => {
                let delta_x = number(request, "dx")?;
                let delta_y = number(request, "dy")?;
                self.camera_orbit(delta_x, delta_y)
            }
            "camera.projection" => {
                let mode = request.get("mode").and_then(Json::as_str);
                let height = request
                    .get("height")
                    .and_then(Json::as_f64)
                    .map(|value| value as f32);
                self.camera_projection(mode, height)
            }

            "selection.set" => self.select(request.get("entity").and_then(Json::as_u64)),
            "selection.pick" => self.pick(
                number(request, "x")?,
                number(request, "y")?,
                number(request, "aspect")?,
            ),

            "camera.look" => {
                self.camera_look(number(request, "dx")?, number(request, "dy")?);
                Ok(Json::Null)
            }
            "camera.advance" => {
                let boost = request
                    .get("boost")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                self.camera_advance(
                    number_or(request, "right", 0.0),
                    number_or(request, "up", 0.0),
                    number_or(request, "forward", 0.0),
                    boost,
                    number(request, "seconds")?,
                );
                Ok(Json::Null)
            }
            "camera.get" => Ok(self.camera_json()),
            "camera.reset" => self.camera_reset().map(|()| Json::Null),
            "camera.focus" => self.camera_focus().map(|()| Json::Null),

            other => Err(Failure::new(
                "EDITOR_UNKNOWN_OP",
                &format!("operação desconhecida `{other}`"),
                "Veja a lista de operações em docs/02-arquitetura/11-editor.md.",
            )),
        }
    }
}

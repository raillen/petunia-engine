use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use petunia_commands::{CommandRecord, Dispatcher};
use petunia_math::Vec3;
use petunia_project::{
    ProjectManifest, Report, Severity, create_sample_project, load_project, save_scene,
};
use petunia_render::{
    FlyCamera, MeshCache, RenderFrame, extract_frame_cached, pick_entity, ray_through,
    start_camera, start_camera_at,
};
use petunia_world::{PersistentId, Value, World};
use serde_json::{Map, Value as Json, json};

use crate::brush_ops::{FaceDrag, FaceHit};
use crate::failure::Failure;
use crate::floors::AboveFloor;
use crate::gizmo::{Drag, Handle, Orientation, Snap, Tool};
use crate::mesh_tool::{ComponentSelection, MeshComponentMode};
use crate::natural_tool::NaturalState;
use crate::sculpt_tool::SculptState;
use crate::tools::{DEFAULT_FLOOR_HEIGHT, FreeMode, PivotMode, SnapTarget};

/// The log keeps the most recent entries only.
const MAX_LOG_ENTRIES: usize = 300;
/// The outline of a huge scene would be a huge message; the list is cut here and says so.
const MAX_LISTED_ENTITIES: usize = 10_000;
/// Meters in front of the camera where new brushes appear.
const CREATE_DISTANCE: f32 = 6.0;
/// New brushes snap to this grid (meters).
const CREATE_SNAP: f32 = 0.5;

pub(crate) fn live_edit_busy(verb: &str) -> Failure {
    Failure::new(
        "EDITOR_BUSY",
        "há um arrasto em andamento",
        &format!("Termine ou cancele o arrasto antes de {verb}."),
    )
}

pub(crate) struct OpenProject {
    pub(crate) root: PathBuf,
    pub(crate) manifest: ProjectManifest,
    pub(crate) scene_path: PathBuf,
    pub(crate) world: World,
}

/// A drag in progress: the values being changed live (columns of one row) and what they were
/// before the drag.
struct Gesture {
    entity: u64,
    /// (table, column, value before the drag)
    originals: Vec<(String, String, Value)>,
}

struct LogEntry {
    number: u64,
    kind: &'static str,
    text: String,
    ok: bool,
    detail: String,
}

/// Everything the editor remembers: the open project, the undo history, the selection and the
/// camera. It never touches a window or the GPU; a GUI drives it through
/// [`EditorSession::handle`] and asks for a [`RenderFrame`] to draw.
pub struct EditorSession {
    pub(crate) project: Option<OpenProject>,
    pub(crate) dispatcher: Dispatcher,
    pub(crate) selection: Option<u64>,
    pub(crate) camera: FlyCamera,
    log: VecDeque<LogEntry>,
    pub(crate) report: Report,
    /// Findings about the project's scripts, kept apart from `report` so a broken script never
    /// blocks opening the project.
    pub(crate) script_diagnostics: Vec<petunia_project::Diagnostic>,
    pub(crate) dirty: bool,
    log_counter: u64,
    gesture: Option<Gesture>,
    pub(crate) tool: Tool,
    pub(crate) snap: Snap,
    pub(crate) hover: Option<Handle>,
    pub(crate) drag: Option<Drag>,
    /// Object under the mouse (pre-selection highlight).
    pub(crate) hovered: Option<u64>,
    /// The only object the viewport shows while the outliner is in "see only this" (`object.solo`).
    pub(crate) solo_object: Option<u64>,
    pub(crate) orientation: Orientation,
    pub(crate) pivot_mode: PivotMode,
    /// Moving the gizmo moves the object's pivot instead of the object.
    pub(crate) edit_pivot: bool,
    pub(crate) snap_target: SnapTarget,
    /// Surface snapping tilts the object with the slope.
    pub(crate) align_to_slope: bool,
    /// The floor being built ("andar", a `Floor` entity; `None` is the implicit ground), the
    /// height for new floors, and how objects above it are drawn.
    pub(crate) floor: Option<u64>,
    pub(crate) floor_height: f32,
    pub(crate) above_floor: AboveFloor,
    pub(crate) free: Option<FreeMode>,
    pub(crate) hovered_face: Option<FaceHit>,
    pub(crate) face_drag: Option<FaceDrag>,
    /// A collider face being resized in the viewport (square handles).
    pub(crate) collider_drag: Option<crate::colliders::ColliderDrag>,
    /// Short text for the heads-up display while dragging.
    pub(crate) hud: Option<String>,
    /// Gizmo rod length in logical pixels.
    pub(crate) gizmo_size: f32,
    /// Color and thickness of the selection outline (editor preference).
    pub(crate) outline: crate::tools::OutlineStyle,
    /// The sculpting tool (caves).
    pub(crate) sculpt: SculptState,
    /// Terrain, river and road tools.
    pub(crate) natural: NaturalState,
    /// Faces, edges and vertices chosen on an editable mesh (`mesh.select`).
    pub(crate) components: Option<ComponentSelection>,
    /// Whether the mesh edit handles (all vertices, for example) are shown.
    pub(crate) mesh_edit_mode: bool,
    /// What a click chooses while editing a mesh.
    pub(crate) mesh_component_mode: MeshComponentMode,
    /// Triangles of editable meshes, reused while they do not change (the tools extract the
    /// scene many times per mouse move).
    pub(crate) mesh_cache: RefCell<MeshCache>,
    /// The running game (`play.start`), on a copy of the world.
    pub(crate) play: Option<Box<crate::play::PlayState>>,
    /// The node graph being edited (`graph.open`).
    pub(crate) graph: Option<crate::graph_ops::OpenGraph>,
}

pub(crate) fn vector_json(vector: Vec3) -> Json {
    json!({ "x": vector.x, "y": vector.y, "z": vector.z })
}

fn text_of(world: &World, id: u64, table: &str, column: &str) -> Option<String> {
    let handle = world.handle_of(PersistentId(id)).ok()?;
    match world.get(handle, table, column) {
        Ok(Value::Text(text)) => Some(text.clone()),
        _ => None,
    }
}

pub(crate) fn vec3_of(world: &World, id: u64, table: &str, column: &str) -> Option<Vec3> {
    let handle = world.handle_of(PersistentId(id)).ok()?;
    match world.get(handle, table, column) {
        Ok(Value::Vec3(vector)) => Some(*vector),
        _ => None,
    }
}

fn camera_at_start(world: &World) -> FlyCamera {
    match petunia_project::player_start(world) {
        Some(start) => start_camera_at(start.eye, start.yaw_degrees),
        None => start_camera(world),
    }
}

pub(crate) fn display_name(world: &World, id: u64) -> String {
    match text_of(world, id, "Name", "text") {
        Some(name) if !name.is_empty() => name,
        _ => format!("Entidade {id}"),
    }
}

pub(crate) fn new_dispatcher() -> Result<Dispatcher, Failure> {
    let mut dispatcher = Dispatcher::with_builtin()?;
    for (spec, apply) in crate::natural_ops::commands() {
        dispatcher.register(spec, apply)?;
    }
    Ok(dispatcher)
}

impl EditorSession {
    pub fn new() -> Result<Self, Failure> {
        Ok(Self {
            project: None,
            dispatcher: new_dispatcher()?,
            selection: None,
            camera: FlyCamera::new(petunia_render::Camera::default()),
            log: VecDeque::new(),
            report: Report::default(),
            script_diagnostics: Vec::new(),
            dirty: false,
            log_counter: 0,
            gesture: None,
            tool: Tool::Select,
            snap: Snap::default(),
            hover: None,
            drag: None,
            hovered: None,
            solo_object: None,
            orientation: Orientation::Global,
            pivot_mode: PivotMode::Object,
            edit_pivot: false,
            snap_target: SnapTarget::Smart,
            align_to_slope: false,
            floor: None,
            floor_height: DEFAULT_FLOOR_HEIGHT,
            above_floor: AboveFloor::Ghost,
            free: None,
            hovered_face: None,
            face_drag: None,
            collider_drag: None,
            hud: None,
            gizmo_size: Self::default_gizmo_size(),
            outline: crate::tools::OutlineStyle::default(),
            sculpt: SculptState::new(),
            natural: NaturalState::default(),
            components: None,
            mesh_edit_mode: false,
            mesh_component_mode: MeshComponentMode::Face,
            mesh_cache: RefCell::new(MeshCache::new()),
            play: None,
            graph: None,
        })
    }

    /// Read access for queries (the GUI never gets a mutable world).
    pub(crate) fn world_for_query(&self) -> Result<&World, Failure> {
        self.world()
    }

    pub(crate) fn world(&self) -> Result<&World, Failure> {
        self.project
            .as_ref()
            .map(|project| &project.world)
            .ok_or_else(Failure::no_project)
    }

    pub(crate) fn record(&mut self, kind: &'static str, text: &str, ok: bool, detail: &str) {
        self.log_counter += 1;
        self.log.push_back(LogEntry {
            number: self.log_counter,
            kind,
            text: text.to_string(),
            ok,
            detail: detail.to_string(),
        });
        while self.log.len() > MAX_LOG_ENTRIES {
            self.log.pop_front();
        }
    }

    // ---- project -------------------------------------------------------------------------

    /// Opens a project folder. When it cannot be opened the current project stays as it was and
    /// the diagnostics explain why.
    pub fn open_project(&mut self, root: &Path) -> Result<Json, Failure> {
        let outcome = load_project(root);
        self.report = outcome.report;
        let blocked = self.report.has_errors();
        let Some(loaded) = outcome.project.filter(|_| !blocked) else {
            let failure = match self
                .report
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.severity == Severity::Error)
            {
                Some(first) => Failure::new(&first.code, &first.message, &first.action),
                None => Failure::new(
                    "PROJECT_NOT_LOADED",
                    "não foi possível abrir o projeto",
                    "Veja os diagnósticos.",
                ),
            };
            self.record(
                "project",
                &format!("abrir {}", root.display()),
                false,
                &failure.message,
            );
            return Err(failure);
        };
        self.camera = camera_at_start(&loaded.world);
        let name = loaded.manifest.name.clone();
        self.reset_live_edit();
        self.project = Some(OpenProject {
            root: loaded.root,
            manifest: loaded.manifest,
            scene_path: loaded.scene_path,
            world: loaded.world,
        });
        self.dispatcher = new_dispatcher()?;
        self.selection = None;
        self.graph = None;
        self.floor = None;
        self.dirty = false;
        self.refresh_script_diagnostics();
        self.record("project", &format!("abrir {name}"), true, "");
        Ok(json!({ "name": name }))
    }

    /// Creates the sample room in a new folder and opens it.
    pub fn new_sample_project(&mut self, root: &Path, name: &str) -> Result<Json, Failure> {
        create_sample_project(root, name)?;
        self.open_project(root)
    }

    /// Saves the scene. Atomic: a crash never leaves a half-written file. Refused while a live
    /// edit is changing the world without a command: the file would keep a transient preview.
    pub fn save(&mut self) -> Result<Json, Failure> {
        if self.has_live_edit() {
            return Err(live_edit_busy("salvar"));
        }
        let project = self.project.as_ref().ok_or_else(Failure::no_project)?;
        save_scene(&project.scene_path, &project.world)?;
        let path = project.scene_path.display().to_string();
        self.dirty = false;
        self.forget_autosave();
        self.record("project", "salvar", true, &path);
        Ok(json!({ "path": path }))
    }

    /// True while a drag is changing the world outside the command history.
    pub(crate) fn has_live_edit(&self) -> bool {
        self.gesture.is_some()
            || self.drag.is_some()
            || self.face_drag.is_some()
            || self.collider_drag.is_some()
            || self.free.is_some()
            || self.sculpt.drag.is_some()
            || self.natural.dragging
            || self
                .components
                .as_ref()
                .is_some_and(ComponentSelection::is_previewing)
    }

    /// Forgets every drag bound to the old world. The world is being replaced, so there is
    /// nothing to restore; keeping the state would let the next commit hit a new entity.
    pub(crate) fn reset_live_edit(&mut self) {
        self.gesture = None;
        self.drag = None;
        self.face_drag = None;
        self.collider_drag = None;
        self.free = None;
        self.hud = None;
        self.hover = None;
        self.hovered = None;
        self.hovered_face = None;
        self.sculpt.reset_transient();
        self.components = None;
        self.mesh_edit_mode = false;
    }

    // ---- commands ------------------------------------------------------------------------

    /// Validates and runs one command; it joins the undo history when reversible.
    pub fn execute(&mut self, record: &CommandRecord) -> Result<Json, Failure> {
        self.ensure_not_playing()?;
        let outcome = {
            let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
            self.dispatcher.execute(&mut project.world, record)
        };
        let label = command_label(record);
        match outcome {
            Ok(result) => {
                self.dirty = true;
                self.drop_stale_selection();
                self.record("command", &label, true, &result.to_string());
                Ok(result)
            }
            Err(error) => {
                self.record("command", &label, false, &error.to_string());
                Err(error.into())
            }
        }
    }

    pub fn undo(&mut self) -> Result<Json, Failure> {
        self.ensure_not_playing()?;
        if self.has_live_edit() {
            return Err(live_edit_busy("desfazer"));
        }
        let outcome = {
            let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
            self.dispatcher.undo(&mut project.world)
        };
        self.after_history_step("desfazer", outcome)
    }

    pub fn redo(&mut self) -> Result<Json, Failure> {
        self.ensure_not_playing()?;
        if self.has_live_edit() {
            return Err(live_edit_busy("refazer"));
        }
        let outcome = {
            let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
            self.dispatcher.redo(&mut project.world)
        };
        self.after_history_step("refazer", outcome)
    }

    fn after_history_step(
        &mut self,
        verb: &str,
        outcome: Result<String, petunia_commands::CommandError>,
    ) -> Result<Json, Failure> {
        match outcome {
            Ok(command) => {
                self.dirty = true;
                self.drop_stale_selection();
                self.record("history", &format!("{verb} {command}"), true, "");
                Ok(json!({ "command": command }))
            }
            Err(error) => {
                self.record("history", verb, false, &error.to_string());
                Err(error.into())
            }
        }
    }

    fn drop_stale_selection(&mut self) {
        let Some(id) = self.selection else {
            return;
        };
        let alive = self
            .project
            .as_ref()
            .is_some_and(|project| project.world.handle_of(PersistentId(id)).is_ok());
        if !alive {
            self.selection = None;
        }
    }

    // ---- gestures (live drag, one undo entry) --------------------------------------------

    /// Changes one value live while the person drags. This is a preview, not a command: nothing
    /// enters the undo history until [`EditorSession::gesture_commit`]. A gesture can touch several
    /// columns of the same row (turning around a pivot changes rotation and position). Starting
    /// on another row commits the previous gesture first.
    pub fn gesture_preview(
        &mut self,
        entity: u64,
        table: &str,
        column: &str,
        value: &Json,
    ) -> Result<Json, Failure> {
        let other_entity = self
            .gesture
            .as_ref()
            .is_some_and(|gesture| gesture.entity != entity);
        if other_entity {
            self.gesture_commit()?;
        }
        let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
        let world = &mut project.world;
        let handle = world.handle_of(PersistentId(entity))?;
        let kind = world.column_kind(table, column)?;
        let parsed =
            Value::from_json(kind, value, world.limits().max_text_bytes).map_err(|reason| {
                petunia_world::WorldError::InvalidValue {
                    column: column.to_string(),
                    reason,
                }
            })?;
        let gesture = self.gesture.get_or_insert_with(|| Gesture {
            entity,
            originals: Vec::new(),
        });
        if !gesture
            .originals
            .iter()
            .any(|(known_table, known_column, _)| known_table == table && known_column == column)
        {
            let original = world.get(handle, table, column)?.clone();
            gesture
                .originals
                .push((table.to_string(), column.to_string(), original));
        }
        world.set(handle, table, column, parsed)?;
        Ok(Json::Null)
    }

    /// Ends the drag: puts the original values back and runs ONE command with the final values
    /// (`world.set` for one value, a `world.batch` of them for several), so a single undo returns
    /// to before the drag. No change means no history entry.
    pub fn gesture_commit(&mut self) -> Result<Json, Failure> {
        let Some(gesture) = self.gesture.take() else {
            return Ok(Json::Null);
        };
        let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
        let world = &mut project.world;
        let handle = world.handle_of(PersistentId(gesture.entity))?;
        let mut records = Vec::new();
        for (table, column, original) in &gesture.originals {
            let finished = world.get(handle, table, column)?.clone();
            world.set(handle, table, column, original.clone())?;
            if finished != *original {
                records.push(CommandRecord::new(
                    "world.set",
                    json!({ "entity": gesture.entity, "table": table, "column": column, "value": finished.to_json() }),
                ));
            }
        }
        let record = match records.len() {
            0 => return Ok(Json::Null),
            1 => records.remove(0),
            _ => CommandRecord::new(
                petunia_commands::BATCH_COMMAND,
                json!({ "records": records }),
            ),
        };
        self.execute(&record)
    }

    /// Abandons the drag and restores the original values.
    pub fn gesture_cancel(&mut self) -> Result<Json, Failure> {
        let Some(gesture) = self.gesture.take() else {
            return Ok(Json::Null);
        };
        let project = self.project.as_mut().ok_or_else(Failure::no_project)?;
        let world = &mut project.world;
        let handle = world.handle_of(PersistentId(gesture.entity))?;
        for (table, column, original) in gesture.originals {
            world.set(handle, &table, &column, original)?;
        }
        Ok(Json::Null)
    }

    // ---- selection and editing helpers ---------------------------------------------------

    pub fn select(&mut self, entity: Option<u64>) -> Result<Json, Failure> {
        if let Some(id) = entity {
            self.world()?.handle_of(PersistentId(id))?;
            // A locked object stays drawn but cannot be chosen: it is a background reference.
            if let Some(world) = self.world().ok()
                && petunia_world::state_of(world, id).locked
            {
                return Err(Failure::new(
                    "EDITOR_OBJECT_LOCKED",
                    &format!("o objeto {id} está travado"),
                    "Destrave-o na Hierarquia para escolhê-lo.",
                ));
            }
        }
        self.selection = entity;
        Ok(json!({ "selection": entity }))
    }

    /// Selects what is under a point of the viewport (`-1..=1`, bottom-left is `-1, -1`), or
    /// clears the selection when the point is empty space.
    pub fn pick(&mut self, ndc_x: f32, ndc_y: f32, aspect_ratio: f32) -> Result<Json, Failure> {
        let frame = self.frame().ok_or_else(Failure::no_project)?;
        let ray = ray_through(&self.camera.camera, aspect_ratio, ndc_x, ndc_y);
        let brush = ray.and_then(|ray| pick_entity(&self.pickable(frame.items), &ray));
        let sculpt = ray.and_then(|ray| self.sculpt_hit(&ray, &[]));
        let hit = match (brush, sculpt) {
            (Some(brush), Some(sculpt)) if sculpt.distance < brush.1 => Some(sculpt.entity),
            (Some(brush), _) => Some(brush.0),
            (None, sculpt) => sculpt.map(|hit| hit.entity),
        };
        self.selection = hit;
        Ok(json!({ "selection": self.selection }))
    }

    /// Adds a brush in front of the camera, sitting on the ground, and selects it.
    pub fn create_brush(&mut self, shape: &str) -> Result<Json, Failure> {
        let (name, size, color, detail) = match shape {
            "box" => ("Caixa", Vec3::new(2.0, 2.0, 2.0), [0.62, 0.55, 0.45], 16),
            "ramp" => ("Rampa", Vec3::new(3.0, 2.0, 4.0), [0.55, 0.60, 0.68], 16),
            "cylinder" => ("Cilindro", Vec3::new(2.0, 3.0, 2.0), [0.68, 0.52, 0.52], 16),
            "cone" => ("Cone", Vec3::new(2.0, 3.0, 2.0), [0.60, 0.68, 0.52], 16),
            "sphere" => ("Esfera", Vec3::new(2.0, 2.0, 2.0), [0.52, 0.62, 0.70], 16),
            "stairs" => ("Escada", Vec3::new(3.0, 2.0, 4.0), [0.66, 0.62, 0.55], 6),
            other => {
                return Err(Failure::bad_request(&format!(
                    "forma `{other}` desconhecida (use box, ramp, cylinder, cone, sphere ou stairs)"
                )));
            }
        };
        let camera = self.camera.camera;
        let forward = camera.forward();
        let flat = Vec3::new(forward.x, 0.0, forward.z).normalized_or_zero();
        let flat = if flat.length_squared() == 0.0 {
            Vec3::new(0.0, 0.0, -1.0)
        } else {
            flat
        };
        let snap = |value: f32| (value / CREATE_SNAP).round() * CREATE_SNAP;
        // In front of the camera, sitting on the floor being built.
        let target = camera.position + flat * CREATE_DISTANCE;
        let position = Vec3::new(
            snap(target.x),
            self.floor_elevation() + size.y * 0.5,
            snap(target.z),
        );
        let record = CommandRecord::new(
            "world.create",
            json!({
                "rows": {
                    "Name": { "text": name },
                    "Transform": { "position": vector_json(position) },
                    "Brush": {
                        "shape": shape,
                        "size": vector_json(size),
                        "detail": detail,
                        "color": { "r": color[0], "g": color[1], "b": color[2], "a": 1.0 }
                    }
                }
            }),
        );
        self.create_and_select(&record)
    }

    /// Copies the selected entity one meter to the side and selects the copy.
    pub fn duplicate(&mut self) -> Result<Json, Failure> {
        let id = self.selection.ok_or_else(no_selection)?;
        let record = {
            let world = self.world()?;
            let handle = world.handle_of(PersistentId(id))?;
            let snapshot = world.snapshot(handle)?;
            let mut rows = Map::new();
            for (table, values) in &snapshot.rows {
                let mut row = world.row_to_json(table, values)?;
                match table.as_str() {
                    "Transform" => {
                        if let Some(position) = vec3_of(world, id, "Transform", "position") {
                            row.insert(
                                "position".to_string(),
                                vector_json(position + Vec3::new(1.0, 0.0, 0.0)),
                            );
                        }
                    }
                    "Name" => {
                        let name = display_name(world, id);
                        row.insert("text".to_string(), json!(format!("{name} (cópia)")));
                    }
                    _ => {}
                }
                rows.insert(table.clone(), Json::Object(row));
            }
            CommandRecord::new(
                "world.create",
                json!({ "tags": snapshot.tags, "rows": rows }),
            )
        };
        // A cave is copied with its strokes and an object with its colliders, pointing at the
        // copy: one batch, one undo step.
        let mut strokes = self.copy_strokes_records(id, 0);
        strokes.extend(self.copy_collider_records(id, 0));
        strokes.extend(self.copy_script_records(id, 0));
        if strokes.is_empty() {
            return self.create_and_select(&record);
        }
        let mut records = vec![record];
        records.extend(strokes);
        let result = self.execute(&CommandRecord::new(
            petunia_commands::BATCH_COMMAND,
            json!({ "records": records }),
        ))?;
        let entity = result
            .get(0)
            .and_then(|first| first.get("entity"))
            .and_then(Json::as_u64);
        self.selection = entity;
        Ok(json!({ "entity": entity }))
    }

    fn create_and_select(&mut self, record: &CommandRecord) -> Result<Json, Failure> {
        let result = self.execute(record)?;
        let entity = result.get("entity").and_then(Json::as_u64);
        self.selection = entity;
        Ok(json!({ "entity": entity }))
    }

    /// Removes the selected entity (undoable).
    pub fn delete_selection(&mut self) -> Result<Json, Failure> {
        let id = self.selection.ok_or_else(no_selection)?;
        // A cave goes with its strokes and an object with its colliders, as one undo step.
        let mut records: Vec<CommandRecord> = self
            .strokes_of(id)
            .into_iter()
            .map(|stroke| CommandRecord::new("world.remove", json!({ "entity": stroke })))
            .collect();
        records.extend(self.remove_collider_records(id));
        records.extend(self.remove_script_records(id));
        records.push(CommandRecord::new("world.remove", json!({ "entity": id })));
        let record = if records.len() == 1 {
            records.remove(0)
        } else {
            CommandRecord::new(
                petunia_commands::BATCH_COMMAND,
                json!({ "records": records }),
            )
        };
        self.execute(&record)?;
        self.selection = None;
        Ok(json!({ "removed": id }))
    }

    // ---- camera --------------------------------------------------------------------------

    pub fn camera_look(&mut self, delta_x: f32, delta_y: f32) {
        self.camera.look(delta_x, delta_y);
    }

    pub fn camera_advance(
        &mut self,
        right: f32,
        up: f32,
        forward: f32,
        boost: bool,
        elapsed_seconds: f32,
    ) {
        if self.camera.camera.projection_mode == petunia_render::ProjectionMode::Orthographic {
            if forward.is_finite() && forward != 0.0 {
                let zoom_speed = if boost { 1.30_f32 } else { 1.15_f32 };
                let factor = zoom_speed.powf(forward.abs());
                let new_height = if forward > 0.0 {
                    self.camera.camera.orthographic_height / factor
                } else {
                    self.camera.camera.orthographic_height * factor
                };
                self.camera.camera.orthographic_height = new_height.clamp(0.1, 10_000.0);
            }
            if (right.is_finite() && right != 0.0) || (up.is_finite() && up != 0.0) {
                self.camera.advance(right, up, 0.0, boost, elapsed_seconds);
            }
        } else {
            self.camera
                .advance(right, up, forward, boost, elapsed_seconds);
        }
    }

    /// Back to the player start (or the origin).
    pub fn camera_reset(&mut self) -> Result<(), Failure> {
        self.camera = camera_at_start(self.world()?);
        Ok(())
    }

    /// Moves the camera back from the selection so it fills the view; keeps the viewing angle.
    pub fn camera_focus(&mut self) -> Result<(), Failure> {
        let id = self.selection.ok_or_else(no_selection)?;
        let world = self.world()?;
        let center = vec3_of(world, id, "Transform", "position").unwrap_or(Vec3::ZERO);
        let extent = match (
            vec3_of(world, id, "Brush", "size"),
            vec3_of(world, id, "Transform", "scale"),
        ) {
            (Some(size), scale) => {
                let scale = scale.unwrap_or(Vec3::ONE);
                (size.x * scale.x)
                    .max(size.y * scale.y)
                    .max(size.z * scale.z)
            }
            _ => 1.0,
        };
        let distance = extent * 1.8 + 3.0;
        let forward = self.camera.camera.forward();
        self.camera.camera.position = center - forward * distance;
        Ok(())
    }

    // ---- views for the GUI ---------------------------------------------------------------

    /// What to draw this frame, with the selection marked. `None` when no project is open.
    pub fn frame(&self) -> Option<RenderFrame> {
        if let Some(frame) = self.play_frame() {
            return Some(frame);
        }
        let world = &self.project.as_ref()?.world;
        let mut frame = {
            let mut cache = self.mesh_cache.borrow_mut();
            let frame = extract_frame_cached(world, self.camera.camera, &mut cache);
            // Meshes no longer in the scene (edited, undone, deleted) leave the cache.
            cache.sweep();
            frame
        };
        self.apply_floor_visibility(&mut frame.items);
        self.apply_object_visibility(&mut frame.items);
        frame.sculpts = self.sculpt_items();
        frame.mark_selected(self.selection);
        frame.mark_hovered(self.hovered.filter(|id| Some(*id) != self.selection));
        frame.overlay = self.outline_overlay(&frame);
        self.sculpt_overlay(&mut frame.overlay);
        self.natural_overlay(&mut frame.overlay);
        self.mesh_overlay(&mut frame.overlay);
        self.collider_overlay(&frame.items, &mut frame.overlay);
        self.light_overlay(&mut frame.overlay);
        frame.show_grid = true;
        frame.grid_height = self.floor_elevation();
        Some(frame)
    }

    /// Position and angles of the editor camera (cheap: the GUI asks for it while moving).
    pub fn camera_json(&self) -> Json {
        let camera = self.camera.camera;
        json!({
            "x": camera.position.x,
            "y": camera.position.y,
            "z": camera.position.z,
            "yaw_degrees": camera.yaw_degrees,
            "pitch_degrees": camera.pitch_degrees,
            "projection_mode": camera.projection_mode.name(),
            "orthographic_height": camera.orthographic_height,
        })
    }

    /// Small summary the GUI refreshes after every change.
    pub fn snapshot(&self) -> Json {
        let camera_json = self.camera_json();
        let full = self.full_report();
        let diagnostics = json!({
            "errors": full.count(Severity::Error),
            "warnings": full.count(Severity::Warning),
            "infos": full.count(Severity::Info),
        });
        let Some(project) = &self.project else {
            return json!({
                "project": null,
                "entities": [],
                "selection": null,
                "can_undo": false,
                "can_redo": false,
                "diagnostics": diagnostics,
                "camera": camera_json,
                "natural": self.natural_json(),
            });
        };
        let world = &project.world;
        let all = world.all_entities();
        let truncated = all.len() > MAX_LISTED_ENTITIES;
        let mut entities = Vec::new();
        for handle in all.into_iter().take(MAX_LISTED_ENTITIES) {
            let Ok(identity) = world.persistent_id(handle) else {
                continue;
            };
            let tables: Vec<String> = world
                .snapshot(handle)
                .map(|snapshot| snapshot.rows.keys().cloned().collect())
                .unwrap_or_default();
            // The strokes of a cave are part of it; listing hundreds of them would bury the scene.
            if tables
                .iter()
                .any(|table| table == "SculptStroke" || table == "TerrainChunk")
            {
                continue;
            }
            // A collider is listed under its object, by its own name.
            if let Some(owner) = crate::colliders::owner_of(world, identity.0) {
                entities.push(json!({
                    "id": identity.0,
                    "name": crate::colliders::collider_name(world, identity.0).unwrap_or_default(),
                    "tables": tables,
                    "owner": owner,
                }));
                continue;
            }
            // A second or later script attachment is a child entity; list it under the object by
            // the file name it runs.
            if let Some(owner) = crate::scripts::script_owner_of(world, identity.0) {
                entities.push(json!({
                    "id": identity.0,
                    "name": crate::scripts::script_file_of(world, identity.0).unwrap_or_default(),
                    "tables": tables,
                    "owner": owner,
                }));
                continue;
            }
            let state = petunia_world::state_of(world, identity.0);
            entities.push(json!({
                "id": identity.0,
                "name": display_name(world, identity.0),
                "tables": tables,
                "hidden": state.hidden,
                "locked": state.locked,
            }));
        }
        json!({
            "project": {
                "name": project.manifest.name,
                "root": project.root.display().to_string(),
                "scene": project
                    .scene_path
                    .strip_prefix(&project.root)
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| project.scene_path.display().to_string()),
                "scripting_language": project.manifest.scripting_language,
                "dirty": self.dirty,
            },
            "entities": entities,
            "entity_count": world.entity_count(),
            // What people count as objects: cave strokes and colliders are parts of something.
            "object_count": world.entity_count().saturating_sub(
                world.entities_with_table("SculptStroke").map_or(0, |strokes| strokes.len())
                    + world.entities_with_table("TerrainChunk").map_or(0, |chunks| chunks.len())
                    + world.entities_with_table("Collider").map_or(0, |colliders| colliders.len())
                    + crate::scripts::script_attachment_count(world)
            ),
            "truncated": truncated,
            "selection": self.selection,
            "can_undo": self.dispatcher.can_undo(),
            "can_redo": self.dispatcher.can_redo(),
            "diagnostics": diagnostics,
            "camera": camera_json,
            "tool": self.tool.name(),
            "playing": self.is_playing(),
            "snap": self.snap_json(),
            "transform": self.transform_json(),
            "floor": self.floor_json(),
            "sculpt": self.sculpt_json(),
            "natural": self.natural_json(),
            "mesh_selection": self.components_json(),
        })
    }

    /// Every table of one entity, with column kinds and current values, for the inspector.
    pub fn describe(&self, entity: u64) -> Result<Json, Failure> {
        let world = self.world()?;
        let handle = world.handle_of(PersistentId(entity))?;
        let snapshot = world.snapshot(handle)?;
        // People look for the name and the transform first; the rest follows alphabetically.
        let mut ordered: Vec<(&String, &Vec<Value>)> = snapshot.rows.iter().collect();
        ordered.sort_by_key(|(table, _)| {
            let rank = match table.as_str() {
                "Name" => 0,
                "Transform" => 1,
                _ => 2,
            };
            (rank, (*table).clone())
        });
        let mut tables = Vec::new();
        for (table, values) in ordered {
            let Some(spec) = world.table_spec(table) else {
                continue;
            };
            let columns: Vec<Json> = spec
                .columns
                .iter()
                .zip(values.iter())
                .map(|(column, value)| {
                    let mut entry = Map::new();
                    entry.insert("name".to_string(), json!(column.name));
                    entry.insert("kind".to_string(), json!(column.kind.as_str()));
                    entry.insert("value".to_string(), value.to_json());
                    let choices: Option<&[&str]> = match (table.as_str(), column.name.as_str()) {
                        ("Brush", "shape") => Some(&petunia_project::BRUSH_SHAPES),
                        ("Collider", "shape") => Some(&petunia_project::COLLIDER_SHAPES),
                        ("RigidBody", "kind") => Some(&petunia_project::BODY_KINDS),
                        ("Light", "kind") => Some(&petunia_project::LIGHT_KINDS),
                        _ => None,
                    };
                    if let Some(choices) = choices {
                        entry.insert("choices".to_string(), json!(choices));
                    }
                    Json::Object(entry)
                })
                .collect();
            tables.push(json!({ "name": table, "columns": columns }));
        }
        // Parts that belong to another object are added from that object ("Adicionar colisão",
        // the sculpting tool), never as a table here: they would point at nothing.
        let addable: Vec<String> = world
            .table_names()
            .into_iter()
            .filter(|name| !snapshot.rows.contains_key(name))
            .filter(|name| !matches!(name.as_str(), "Collider" | "SculptStroke"))
            .collect();
        Ok(json!({
            "id": entity,
            "name": crate::colliders::collider_name(world, entity)
                .unwrap_or_else(|| display_name(world, entity)),
            "tags": snapshot.tags,
            "tables": tables,
            "addable_tables": addable,
        }))
    }

    /// The Project Doctor's findings: the project's own, then the scripts'.
    fn full_report(&self) -> Report {
        let mut report = self.report.clone();
        report.extend(self.script_diagnostics.iter().cloned());
        report
    }

    pub fn diagnostics(&self) -> Json {
        serde_json::to_value(self.full_report()).unwrap_or(Json::Null)
    }

    pub fn history(&self) -> Json {
        let entries: Vec<Json> = self
            .log
            .iter()
            .map(|entry| {
                json!({
                    "number": entry.number,
                    "kind": entry.kind,
                    "text": entry.text,
                    "ok": entry.ok,
                    "detail": entry.detail,
                })
            })
            .collect();
        json!({ "entries": entries })
    }

    pub fn schema(&self) -> Json {
        serde_json::to_value(self.dispatcher.schema()).unwrap_or(Json::Null)
    }
}

fn no_selection() -> Failure {
    Failure::new(
        "EDITOR_NO_SELECTION",
        "nenhuma entidade está selecionada",
        "Clique em um objeto no viewport ou escolha um item na Hierarquia.",
    )
}

/// `world.set {…}` shortened for the log.
fn command_label(record: &CommandRecord) -> String {
    let payload = record.payload.to_string();
    let shortened: String = payload.chars().take(120).collect();
    if shortened.len() < payload.len() {
        format!("{} {shortened}…", record.command)
    } else {
        format!("{} {shortened}", record.command)
    }
}

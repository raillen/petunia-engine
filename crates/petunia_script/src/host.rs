//! The Script Host: loads scripts, gives each its own VM, and runs their handlers against the
//! world. Everything a script can do is listed in [`crate::api`]; everything it is allowed to do is
//! a capability; and every call runs under an instruction budget, so a script cannot freeze the
//! game.

use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use aipo_bytecode::BytecodeModule;
use aipo_host::{Capability, CapabilitySet};
use aipo_sema::PreludeSurface;
use aipo_vm::{AipoEngine, HostNativeCallback, Value, VmError, VmFault};
use petunia_commands::{CommandRecord, Dispatcher};
use petunia_world::{PersistentId, World};
use serde_json::{Value as Json, json};

use crate::api::{API, CONTEXT_GLOBAL, GAME_CAPABILITIES, install_standard, script_surface};
use crate::compile::{MAX_SCRIPT_BYTES, ScriptDiagnostic, compile};
use crate::natives::{self, DebugEvent, GameCell, GameState, LogLevel, LogLine};
pub use crate::physics_api::PlayerInput;
use crate::physics_api::{self, PhysicsRuntime};

/// Scripts of a project live here, one `.aipo` file each; a `Script` row names one by its file
/// name without the extension.
pub const SCRIPTS_DIRECTORY: &str = "scripts";
pub const SCRIPT_EXTENSION: &str = "aipo";
/// A project has at most this many scripts loaded.
const MAX_SCRIPTS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptLimits {
    /// Instructions one call (a handler, or a script's top level) may run before it is stopped.
    pub instructions_per_call: u64,
}

impl Default for ScriptLimits {
    fn default() -> Self {
        // About 10 ms of the VM at its measured ~100 ns per instruction.
        Self {
            instructions_per_call: 100_000,
        }
    }
}

/// A handler that failed. The object's script is switched off until the file is reloaded, so one
/// bug does not become a thousand log lines.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptFault {
    pub entity: Option<u64>,
    pub script: String,
    pub function: String,
    pub code: String,
    pub message: String,
}

/// What [`ScriptHost::load_directory`] found.
#[derive(Debug, Default)]
pub struct LoadReport {
    pub loaded: Vec<String>,
    pub failed: Vec<(String, Vec<ScriptDiagnostic>)>,
}

fn placeholder(_args: &[Value]) -> Result<Value, VmFault> {
    Ok(Value::None)
}

/// The native behind an API entry, by module and name.
fn callback_for(module: &str, name: &str) -> Option<HostNativeCallback> {
    Some(match (module, name) {
        ("world", "get") => natives::world_get,
        ("world", "row") => natives::world_row,
        ("world", "has") => natives::world_has,
        ("world", "exists") => natives::world_exists,
        ("world", "find") => natives::world_find,
        ("world", "set") => natives::world_set,
        ("world", "create") => natives::world_create,
        ("world", "remove") => natives::world_remove,
        ("transform", "position") => natives::transform_get_position,
        ("transform", "set_position") => natives::transform_set_position,
        ("transform", "move") => natives::transform_move,
        ("transform", "move_x") => natives::transform_move_x,
        ("transform", "move_y") => natives::transform_move_y,
        ("transform", "move_z") => natives::transform_move_z,
        ("transform", "rotation") => natives::transform_get_rotation,
        ("transform", "set_rotation") => natives::transform_set_rotation,
        ("transform", "rotate") => natives::transform_rotate,
        ("transform", "rotate_x") => natives::transform_rotate_x,
        ("transform", "rotate_y") => natives::transform_rotate_y,
        ("transform", "rotate_z") => natives::transform_rotate_z,
        ("transform", "scale") => natives::transform_get_scale,
        ("transform", "set_scale") => natives::transform_set_scale,
        ("transform", "forward") => natives::transform_forward,
        ("transform", "right") => natives::transform_right,
        ("transform", "up") => natives::transform_up,
        ("game", "tick") => natives::game_tick,
        ("game", "seconds") => natives::game_seconds,
        ("game", "emit") => natives::game_emit,
        ("log", "debug") => natives::log_debug,
        ("log", "info") => natives::log_info,
        ("log", "warning") => natives::log_warning,
        ("log", "error") => natives::log_error,
        ("character", "move") => physics_api::character_move,
        ("character", "jump") => physics_api::character_jump,
        ("character", "grounded") => physics_api::character_grounded,
        ("interact", "prompt") => natives::interact_prompt,
        ("health", "current") => natives::health_current,
        ("health", "damage") => natives::health_damage,
        ("health", "heal") => natives::health_heal,
        ("collision", "list") => physics_api::collision_list,
        ("collision", "get") => physics_api::collision_get,
        ("collision", "set") => physics_api::collision_set,
        ("collision", "overlaps") => physics_api::collision_overlaps,
        ("physics", "raycast") => physics_api::physics_raycast,
        ("physics", "raycast_from") => physics_api::physics_raycast_from,
        ("physics", "velocity") => physics_api::physics_velocity,
        ("physics", "set_velocity") => physics_api::physics_set_velocity,
        ("physics", "impulse") => physics_api::physics_impulse,
        ("graph_debug", "node") => natives::graph_debug_node,
        ("graph_debug", "check") => natives::graph_debug_check,
        ("graph_debug", "value") => natives::graph_debug_value,
        _ => return None,
    })
}

/// Every API entry has a native (checked by a test).
#[cfg(test)]
pub(crate) fn api_is_complete() -> bool {
    API.iter()
        .chain(crate::api::DEBUG_API)
        .all(|function| callback_for(function.module, function.name).is_some())
}

fn install_api(engine: &mut AipoEngine) {
    let mut modules: BTreeMap<&str, Vec<(Value, Value)>> = BTreeMap::new();
    for function in API.iter().chain(crate::api::DEBUG_API) {
        let Some(callback) = callback_for(function.module, function.name) else {
            continue;
        };
        let arity = if function.variadic {
            usize::MAX
        } else {
            function.params.len()
        };
        let full = format!("{}.{}", function.module, function.name);
        let vm = engine.vm_mut();
        vm.register_host_native(full.clone(), arity, callback);
        modules.entry(function.module).or_default().push((
            Value::string(function.name),
            Value::native(full, arity, placeholder),
        ));
    }
    for (module, entries) in modules {
        let dict = aipo_vm::DictMap::from_entries(entries);
        engine
            .vm_mut()
            .define_global(module, Value::Dict(Rc::new(RefCell::new(dict))));
    }
    // `print` and `println` write to the console like `log.info` (the standard ones write to the
    // process output, which a game does not have).
    engine.register_host_fn("print", usize::MAX, natives::log_info);
    engine.register_host_fn("println", usize::MAX, natives::log_info);
}

pub struct ScriptHost {
    state: GameCell,
    surface: PreludeSurface,
    scripts: BTreeMap<String, Arc<BytecodeModule>>,
    engines: BTreeMap<(u64, String), AipoEngine>,
    started: BTreeSet<(u64, String)>,
    disabled: BTreeSet<(u64, String)>,
    faults: Vec<ScriptFault>,
    limits: ScriptLimits,
    /// Whether graphs are compiled with the marks play mode reads (N5).
    debug_marks: bool,
    /// The interact key was down on the previous frame, so a hold does not fire again.
    interact_held: bool,
    /// `Health.current` at the end of the previous frame, to notice a total that reaches zero.
    health_points: std::collections::BTreeMap<u64, f64>,
    /// A `Shot` the player chose with interact. Pressing it again clears the choice.
    view_target: Option<u64>,
}

/// The camera row of the keyboard-driven character, plus where that character stands.
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerRig {
    pub mode: String,
    pub distance: f32,
    pub height: f32,
    pub fov_degrees: f32,
    pub feet: petunia_math::Vec3,
    pub yaw_degrees: f32,
}

impl ScriptHost {
    /// A host that owns `world`; scripts change it through the command dispatcher.
    pub fn new(world: World) -> Result<Self, String> {
        let dispatcher = Dispatcher::with_builtin().map_err(|error| error.to_string())?;
        let health_points = crate::gameplay::health_points(&world);
        Ok(Self {
            state: GameCell(Rc::new(RefCell::new(GameState {
                world,
                dispatcher,
                seconds: 0.0,
                commands_run: 0,
                log: Vec::new(),
                events: Vec::new(),
                debug: crate::natives::DebugTrace::default(),
                physics: None,
            }))),
            surface: script_surface(),
            scripts: BTreeMap::new(),
            engines: BTreeMap::new(),
            started: BTreeSet::new(),
            disabled: BTreeSet::new(),
            faults: Vec::new(),
            limits: ScriptLimits::default(),
            debug_marks: false,
            interact_held: false,
            health_points,
            view_target: None,
        })
    }

    pub fn set_limits(&mut self, limits: ScriptLimits) {
        self.limits = limits;
    }

    pub fn limits(&self) -> ScriptLimits {
        self.limits
    }

    /// Turns the marks generated graph code writes on or off (play mode, N5). Off compiles exactly
    /// the text [`crate::graph::compile_graph`] gives.
    pub fn set_debug_marks(&mut self, on: bool) {
        self.debug_marks = on;
    }

    /// What the newest frame reported: the nodes that ran, what each `Se` decided and the values
    /// the condition was made of. Empty when marks are off or nothing has run yet.
    pub fn debug_snapshot(&self) -> Json {
        let (tick, events) = self.state.0.borrow().debug_latest();
        let marks: Vec<Json> = events
            .iter()
            .map(|event| match event {
                DebugEvent::Ran { entity, node } => {
                    json!({ "kind": "ran", "entity": entity, "node": node })
                }
                DebugEvent::Check {
                    entity,
                    node,
                    value,
                } => json!({ "kind": "check", "entity": entity, "node": node, "value": value }),
                DebugEvent::Value { entity, node, text } => {
                    json!({ "kind": "value", "entity": entity, "node": node, "text": text })
                }
            })
            .collect();
        json!({ "tick": tick, "marks": marks })
    }

    /// The world the scripts act on.
    pub fn world(&self) -> Ref<'_, World> {
        Ref::map(self.state.0.borrow(), |state| &state.world)
    }

    /// The world, for the host's own changes between frames.
    pub fn world_mut(&self) -> RefMut<'_, World> {
        RefMut::map(self.state.0.borrow_mut(), |state| &mut state.world)
    }

    /// Commands the scripts ran so far.
    pub fn commands_run(&self) -> u64 {
        self.state.0.borrow().commands_run
    }

    /// What scripts wrote to the console since the last call.
    pub fn take_log(&self) -> Vec<LogLine> {
        std::mem::take(&mut self.state.0.borrow_mut().log)
    }

    /// Handlers that failed since the last call.
    pub fn take_faults(&mut self) -> Vec<ScriptFault> {
        std::mem::take(&mut self.faults)
    }

    /// Turns physics on for this game: builds the simulation from the world's `Collider` and
    /// `RigidBody` tables. `solids` is the drawn surface of each object (`petunia_render::solid_shapes`):
    /// objects with a shape and no collider become solid by themselves. Returns the problems:
    /// colliders and bodies left out, each naming the object; they are also written to the console.
    pub fn attach_physics(
        &mut self,
        settings: petunia_physics::PhysicsSettings,
        solids: BTreeMap<u64, petunia_math::SolidShape>,
    ) -> Result<Vec<String>, String> {
        let mut state = self.state.0.borrow_mut();
        let (runtime, problems) = PhysicsRuntime::new(&state.world, settings, solids)?;
        state.physics = Some(runtime);
        for problem in &problems {
            state.push_log(LogLevel::Warning, format!("física: {problem}"));
        }
        Ok(problems)
    }

    /// Tells the player's character what the person is doing (see [`PlayerInput`]). Call it before
    /// [`ScriptHost::update`] each frame.
    pub fn set_player_input(&mut self, input: PlayerInput) {
        let rising = input.interact && !self.interact_held;
        self.interact_held = input.interact;
        {
            let mut state = self.state.0.borrow_mut();
            let GameState { world, physics, .. } = &mut *state;
            if let Some(runtime) = physics {
                runtime.drive_players(world, input);
            }
        }
        if rising {
            self.queue_interact();
        }
    }

    /// The text of the nearest `Interact` in range, for the line the player reads before pressing.
    pub fn interact_prompt(&self) -> Option<String> {
        crate::gameplay::nearest_interact(&self.state.0.borrow().world).map(|offer| offer.prompt)
    }

    fn queue_interact(&mut self) {
        let offer = crate::gameplay::nearest_interact(&self.state.0.borrow().world);
        let Some(offer) = offer else {
            return;
        };
        let mut state = self.state.0.borrow_mut();
        if state.events.len() >= crate::natives::MAX_QUEUED_EVENTS {
            return;
        }
        state.events.push(crate::natives::QueuedEvent {
            name: "interact".to_string(),
            entity: offer.entity,
        });
        let has_shot = petunia_world::shot_of(&state.world, offer.entity).is_some();
        drop(state);
        if has_shot {
            self.view_target = if self.view_target == Some(offer.entity) {
                None
            } else {
                Some(offer.entity)
            };
        }
    }

    /// The fixed camera the player last chose by interacting with it.
    pub fn watched_shot(&self) -> Option<petunia_world::ShotPose> {
        let entity = self.view_target?;
        petunia_world::shot_of(&self.state.0.borrow().world, entity)
    }

    /// How the keyboard-driven character wants to be seen (DR-43). `mode` is `fixed`
    /// (room shots win, then a chase), `third`, `first` or `isometric`.
    pub fn player_rig(&self) -> Option<PlayerRig> {
        let state = self.state.0.borrow();
        let world = &state.world;
        let handle = world
            .entities_with_table("Character")
            .ok()?
            .into_iter()
            .find(|handle| {
                matches!(
                    world.get(*handle, "Character", "player"),
                    Ok(petunia_world::Value::Bool(true))
                )
            })?;
        let pose = petunia_world::composed_transform(world, handle);
        let number = |column: &str, fallback: f32| match world.get(handle, "Camera", column) {
            Ok(petunia_world::Value::Float(value)) if value.is_finite() && *value > 0.0 => {
                *value as f32
            }
            _ => fallback,
        };
        let mode = match world.get(handle, "Camera", "mode") {
            Ok(petunia_world::Value::Text(mode)) => mode.clone(),
            _ => "fixed".to_string(),
        };
        let enabled = !matches!(
            world.get(handle, "Camera", "enabled"),
            Ok(petunia_world::Value::Bool(false))
        );
        Some(PlayerRig {
            mode: if enabled { mode } else { "fixed".to_string() },
            distance: number("distance", 4.5),
            height: number("height", 1.5),
            fov_degrees: number("fov_degrees", 50.0),
            feet: pose.position,
            yaw_degrees: pose.rotation_degrees.y,
        })
    }

    /// Where the first keyboard-driven character is (for a camera that follows it).
    pub fn player_position(&self) -> Option<petunia_math::Vec3> {
        let state = self.state.0.borrow();
        let world = &state.world;
        world
            .entities_with_table("Character")
            .unwrap_or_default()
            .into_iter()
            .find(|handle| {
                matches!(
                    world.get(*handle, "Character", "player"),
                    Ok(petunia_world::Value::Bool(true))
                )
            })
            .map(|handle| petunia_world::composed_transform(world, handle).position)
    }

    /// Where a third-person camera goes: `distance` meters behind the player's head (`height`
    /// above its position) along `-forward`, pulled in front of anything solid in the way so the
    /// camera never ends up inside a wall.
    pub fn third_person_camera(
        &self,
        forward: petunia_math::Vec3,
        distance: f32,
        height: f32,
    ) -> Option<petunia_math::Vec3> {
        let state = self.state.0.borrow();
        let world = &state.world;
        let (id, position) = world
            .entities_with_table("Character")
            .unwrap_or_default()
            .into_iter()
            .find(|handle| {
                matches!(
                    world.get(*handle, "Character", "player"),
                    Ok(petunia_world::Value::Bool(true))
                )
            })
            .and_then(|handle| {
                let id = world.persistent_id(handle).ok()?.0;
                let pose = petunia_world::composed_transform(world, handle);
                Some((id, pose.position))
            })?;
        let head = position + petunia_math::Vec3::new(0.0, height, 0.0);
        let back = forward * -1.0;
        let mut length = distance;
        if let Some(runtime) = state.physics.as_ref()
            && let Ok(Some(hit)) =
                runtime
                    .simulation
                    .raycast_ignoring(head, back, distance, Some(id))
        {
            length = (hit.distance - 0.3).clamp(0.3, distance);
        }
        Some(head + back * length)
    }

    pub fn has_physics(&self) -> bool {
        self.state.0.borrow().physics.is_some()
    }

    pub fn loaded_scripts(&self) -> Vec<String> {
        self.scripts.keys().cloned().collect()
    }

    /// Gives the world back (the scripts are dropped first, so nothing else holds it).
    pub fn into_world(self) -> World {
        let Self {
            state,
            scripts,
            engines,
            ..
        } = self;
        drop(scripts);
        drop(engines);
        match Rc::try_unwrap(state.0) {
            Ok(cell) => cell.into_inner().world,
            Err(shared) => shared.borrow().world.clone(),
        }
    }

    fn build_engine(&self) -> Result<AipoEngine, String> {
        let mut capabilities = CapabilitySet::none();
        for name in GAME_CAPABILITIES {
            let capability = Capability::parse(name).map_err(|error| error.to_string())?;
            capabilities.grant(capability);
        }
        let mut engine = AipoEngine::builder()
            .capabilities(capabilities)
            .max_instructions(self.limits.instructions_per_call)
            .build()
            .map_err(|fault| fault.to_string())?;
        let mut registry = aipo_runtime::NativeRegistry::new();
        install_standard(engine.vm_mut(), &mut registry);
        install_api(&mut engine);
        let handle = engine
            .vm_mut()
            .host_context()
            .hand_out_userdata(self.state.clone());
        engine
            .vm_mut()
            .define_global(CONTEXT_GLOBAL, Value::HostHandle(handle));
        Ok(engine)
    }

    /// Compiles and loads a script, replacing an earlier version of the same name (hot reload).
    /// Returns the warnings, or every diagnostic when there is an error. A reload switches the
    /// script back on for the objects it had stopped.
    pub fn load(
        &mut self,
        name: &str,
        text: &str,
    ) -> Result<Vec<ScriptDiagnostic>, Vec<ScriptDiagnostic>> {
        if !self.scripts.contains_key(name) && self.scripts.len() >= MAX_SCRIPTS {
            return Err(vec![ScriptDiagnostic::general(
                "SCRIPT_TOO_MANY",
                format!("o projeto já carregou {MAX_SCRIPTS} scripts"),
            )]);
        }
        let compiled = compile(&format!("{name}.{SCRIPT_EXTENSION}"), text, &self.surface);
        let Some(module) = compiled.module else {
            return Err(compiled.diagnostics);
        };
        // The top level runs when the script is first used by an object, not here: with one VM per
        // attachment, running it at load would execute it once more than the object needs.
        self.scripts.insert(name.to_string(), Arc::new(module));
        self.engines.retain(|(_, file), _| file != name);
        self.faults.retain(|fault| fault.script != name);
        let attached = self.attached();
        for (entity, file) in attached {
            if file == name {
                self.disabled.remove(&(entity, file));
            }
        }
        Ok(compiled.diagnostics)
    }

    /// Compiles and loads a node graph as a script. The graph becomes Aipo text in memory; a
    /// problem in that text is reported on the node that wrote the line.
    pub fn load_graph(
        &mut self,
        name: &str,
        text: &str,
    ) -> Result<Vec<ScriptDiagnostic>, Vec<ScriptDiagnostic>> {
        let compiled = crate::graph::compile_graph_with(text, self.debug_marks)?;
        let mut notes = crate::graph::warnings_of(&compiled);
        match self.load(name, &compiled.text) {
            Ok(more) => {
                notes.extend(more.iter().map(|d| crate::graph::blame_node(&compiled, d)));
                Ok(notes)
            }
            Err(diagnostics) => Err(diagnostics
                .iter()
                .map(|d| crate::graph::blame_node(&compiled, d))
                .collect()),
        }
    }

    /// Loads every file of `directory` (not recursive) that belongs to the project's programming
    /// method: `.aipo` for `aipo`/`luau`, `.graph.json` for `nodes` (DR-40). A file that does not
    /// compile is reported and skipped; the rest still load.
    pub fn load_directory(&mut self, directory: &Path, programming_method: &str) -> LoadReport {
        if programming_method == "nodes" {
            return self.load_graphs_of_directory(directory);
        }
        let mut report = LoadReport::default();
        let Ok(entries) = std::fs::read_dir(directory) else {
            return report;
        };
        let mut files: Vec<_> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(SCRIPT_EXTENSION))
            .collect();
        files.sort();
        for path in files {
            let Some(name) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let text = match std::fs::metadata(&path) {
                Ok(meta) if meta.len() as usize > MAX_SCRIPT_BYTES => {
                    report.failed.push((
                        name,
                        vec![ScriptDiagnostic::general(
                            "SCRIPT_TOO_BIG",
                            format!("o arquivo tem mais de {MAX_SCRIPT_BYTES} bytes"),
                        )],
                    ));
                    continue;
                }
                _ => std::fs::read_to_string(&path),
            };
            match text {
                Ok(text) => match self.load(&name, &text) {
                    Ok(_) => report.loaded.push(name),
                    Err(diagnostics) => report.failed.push((name, diagnostics)),
                },
                Err(error) => report.failed.push((
                    name,
                    vec![ScriptDiagnostic::general("SCRIPT_READ", error.to_string())],
                )),
            }
        }
        report
    }

    /// Loads every `.graph.json` of `directory`, for a pNodes project (DR-40). Used instead of the
    /// `.aipo` scan: a pNodes project runs its graphs, not leftover scripts of the other method.
    fn load_graphs_of_directory(&mut self, directory: &Path) -> LoadReport {
        let mut report = LoadReport::default();
        for name in crate::graph::graph_names(directory) {
            let taken =
                report.loaded.contains(&name) || report.failed.iter().any(|(n, _)| *n == name);
            if taken {
                report.failed.push((
                    name.clone(),
                    vec![ScriptDiagnostic::general(
                        "GRAPH_NAME_CLASH",
                        format!(
                            "existem `{name}.{SCRIPT_EXTENSION}` e `{name}{}`; apague ou renomeie um dos dois",
                            crate::graph::GRAPH_SUFFIX
                        ),
                    )],
                ));
                continue;
            }
            let path = directory.join(format!("{name}{}", crate::graph::GRAPH_SUFFIX));
            let too_big = std::fs::metadata(&path)
                .is_ok_and(|meta| meta.len() as usize > petunia_graph::MAX_FILE_BYTES);
            if too_big {
                report.failed.push((
                    name,
                    vec![ScriptDiagnostic::general(
                        "GRAPH_TOO_BIG",
                        format!(
                            "o grafo tem mais de {} bytes",
                            petunia_graph::MAX_FILE_BYTES
                        ),
                    )],
                ));
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(text) => match self.load_graph(&name, &text) {
                    Ok(_) => report.loaded.push(name),
                    Err(diagnostics) => report.failed.push((name, diagnostics)),
                },
                Err(error) => report.failed.push((
                    name,
                    vec![ScriptDiagnostic::general("GRAPH_READ", error.to_string())],
                )),
            }
        }
        report
    }

    /// Objects that have a script, as `(entity, script name)`, in execution order: object id,
    /// then the attachment's `order`, then the file name. An attachment can live on the object's
    /// own `Script` row (legacy, owner 0) or on a child entity with `owner = object id`.
    pub fn attached(&self) -> Vec<(u64, String)> {
        use petunia_world::Value as WorldValue;
        let state = self.state.0.borrow();
        let Ok(handles) = state.world.entities_with_table("Script") else {
            return Vec::new();
        };
        let mut attached: Vec<(u64, i64, String, u64)> = handles
            .into_iter()
            .filter_map(|handle| {
                let id = state.world.persistent_id(handle).ok()?.0;
                let enabled = !matches!(
                    state.world.get(handle, "Script", "enabled"),
                    Ok(WorldValue::Bool(false))
                );
                let Ok(WorldValue::Text(file)) = state.world.get(handle, "Script", "file") else {
                    return None;
                };
                let owner = match state.world.get(handle, "Script", "owner") {
                    Ok(WorldValue::Int(owner)) if *owner > 0 => *owner as u64,
                    _ => id,
                };
                let order = match state.world.get(handle, "Script", "order") {
                    Ok(WorldValue::Int(order)) => *order,
                    _ => 0,
                };
                // A child attachment whose object was removed directly (a script calling
                // world.remove) must not keep running.
                if state.world.handle_of(PersistentId(owner)).is_err() {
                    return None;
                }
                (enabled && !file.is_empty()).then(|| (owner, order, file.clone(), id))
            })
            .collect();
        attached.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });
        attached
            .into_iter()
            .map(|(owner, _, file, _)| (owner, file))
            .collect()
    }

    /// Calls `function` of a script. `Ok(None)` when the script does not define it.
    fn call(
        &mut self,
        entity: u64,
        script: &str,
        function: &str,
        arguments: &[Value],
    ) -> Result<Option<Value>, (String, String)> {
        let limit = self.limits.instructions_per_call;
        let Some(module) = self.scripts.get(script).cloned() else {
            return Err((
                "SCRIPT_NOT_LOADED".to_string(),
                format!("o script `{script}` não está carregado"),
            ));
        };
        let key = (entity, script.to_string());
        if !self.engines.contains_key(&key) {
            let mut engine = self
                .build_engine()
                .map_err(|message| ("SCRIPT_HOST".to_string(), message))?;
            engine.register_module_declarations(&module);
            engine.reset_instruction_count();
            if let Err(error) = engine.run(&module) {
                return Err(describe(&error, limit));
            }
            self.engines.insert(key.clone(), engine);
        }
        let Some(loaded) = self.engines.get_mut(&key) else {
            return Err((
                "SCRIPT_HOST".to_string(),
                "o estado do script desapareceu".to_string(),
            ));
        };
        let Some(callee) = loaded.vm().get_global(function).cloned() else {
            return Ok(None);
        };
        if !matches!(callee, Value::Function { .. } | Value::Closure(_)) {
            return Ok(None);
        }
        loaded.set_instruction_budget(Some(limit));
        loaded.reset_instruction_count();
        match loaded.vm_mut().invoke(&module, callee, arguments) {
            // A handler that ends in an unhandled `Failure` did not do its job: say so, instead of
            // letting it pass for success.
            Ok(Value::Failure(failure)) => Err((
                "SCRIPT_FAILURE".to_string(),
                format!("terminou com uma falha não tratada: {}", failure.message),
            )),
            Ok(value) => Ok(Some(value)),
            Err(error) => Err(describe(&error, limit)),
        }
    }

    fn record_fault(
        &mut self,
        entity: u64,
        script: &str,
        function: &str,
        code: String,
        message: String,
    ) {
        self.disabled.insert((entity, script.to_string()));
        self.state.0.borrow_mut().push_log(
            LogLevel::Error,
            format!("{script}.{function}: {message} (o script do objeto {entity} foi desligado)"),
        );
        self.faults.push(ScriptFault {
            entity: Some(entity),
            script: script.to_string(),
            function: function.to_string(),
            code,
            message,
        });
    }

    fn call_handler(&mut self, entity: u64, script: &str, function: &str, extra: &[Value]) {
        let mut arguments = vec![Value::Int(entity as i64)];
        arguments.extend_from_slice(extra);
        if let Err((code, message)) = self.call(entity, script, function, &arguments) {
            self.record_fault(entity, script, function, code, message);
        }
    }

    /// One frame: `on_start` for objects seen for the first time, `on_update(entity, dt)` for every
    /// object with a script, then the events scripts asked for during the frame.
    pub fn update(&mut self, dt: f64) {
        {
            let mut state = self.state.0.borrow_mut();
            state.seconds += dt;
            // Physics first, so `on_update` sees where things fell this frame.
            let GameState { world, physics, .. } = &mut *state;
            let problems = match physics {
                Some(runtime) => runtime.advance(world, dt),
                None => Vec::new(),
            };
            for problem in problems {
                state.push_log(LogLevel::Warning, format!("física: {problem}"));
            }
        }
        for (entity, script) in self.attached() {
            if self.disabled.contains(&(entity, script.clone())) {
                continue;
            }
            if self.started.insert((entity, script.clone())) {
                self.call_handler(entity, &script, "on_start", &[]);
                if self.disabled.contains(&(entity, script.clone())) {
                    continue;
                }
            }
            self.call_handler(entity, &script, "on_update", &[Value::Float(dt)]);
        }
        let events = std::mem::take(&mut self.state.0.borrow_mut().events);
        let mut attached: BTreeMap<u64, Vec<String>> = BTreeMap::new();
        for (entity, script) in self.attached() {
            attached.entry(entity).or_default().push(script);
        }
        for event in events {
            for script in attached.get(&event.entity).cloned().unwrap_or_default() {
                if self.disabled.contains(&(event.entity, script.clone())) {
                    continue;
                }
                self.call_handler(
                    event.entity,
                    &script,
                    "on_event",
                    &[Value::string(event.name.clone())],
                );
            }
        }
        let after = crate::gameplay::health_points(&self.state.0.borrow().world);
        let died = crate::gameplay::health_deaths(&self.health_points, &after);
        self.health_points = after;
        for entity in died {
            for script in attached.get(&entity).cloned().unwrap_or_default() {
                if self.disabled.contains(&(entity, script.clone())) {
                    continue;
                }
                self.call_handler(entity, &script, "on_event", &[Value::string("died")]);
            }
        }
        self.deliver_contacts();
        self.state.0.borrow_mut().world.advance_tick();
    }

    /// `on_collision(entity, other, mine, theirs)` when a contact starts and `on_collision_end`
    /// when it stops, to the scripts of both objects, in a fixed order.
    fn deliver_contacts(&mut self) {
        let contacts = match self.state.0.borrow_mut().physics.as_mut() {
            Some(runtime) => std::mem::take(&mut runtime.contacts),
            None => return,
        };
        if contacts.is_empty() {
            return;
        }
        let mut attached: BTreeMap<u64, Vec<String>> = BTreeMap::new();
        for (entity, script) in self.attached() {
            attached.entry(entity).or_default().push(script);
        }
        for contact in contacts {
            let handler = match contact.phase {
                petunia_physics::ContactPhase::Started => "on_collision",
                petunia_physics::ContactPhase::Stopped => "on_collision_end",
            };
            for (mine, theirs) in [
                (&contact.first, &contact.second),
                (&contact.second, &contact.first),
            ] {
                if mine.owner == theirs.owner {
                    continue;
                }
                for script in attached.get(&mine.owner).cloned().unwrap_or_default() {
                    if self.disabled.contains(&(mine.owner, script.clone())) {
                        continue;
                    }
                    self.call_handler(
                        mine.owner,
                        &script,
                        handler,
                        &[
                            Value::Int(theirs.owner as i64),
                            Value::string(mine.name.clone()),
                            Value::string(theirs.name.clone()),
                        ],
                    );
                }
            }
        }
    }

    /// Sends an event to one object from the host (a key, a trigger).
    pub fn send_event(&mut self, entity: u64, name: &str) {
        for (owner, script) in self.attached() {
            if owner != entity || self.disabled.contains(&(entity, script.clone())) {
                continue;
            }
            self.call_handler(entity, &script, "on_event", &[Value::string(name)]);
        }
    }
}

/// The code and message of a failed call. Aipo reports an exhausted budget as an integer overflow
/// with a message; here it gets its own code and an explanation a person can act on.
fn describe(error: &VmError, limit: u64) -> (String, String) {
    let message = error.to_string();
    if message.contains("execution budget exceeded") {
        return (
            "SCRIPT_BUDGET_EXCEEDED".to_string(),
            format!(
                "passou do limite de {limit} instruções por chamada (um laço sem fim?). \
                 Divida o trabalho em vários quadros ou use `world.find`."
            ),
        );
    }
    (error.diagnostic_code().to_string(), message)
}

/// What a trial run changed in a copy of the world.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RunReport {
    pub ticks: u32,
    pub commands: u64,
    pub created: usize,
    pub removed: usize,
    pub changed: usize,
    pub log: Vec<(String, u64, String)>,
    pub faults: Vec<ScriptFault>,
}

/// Runs `scripts` for `ticks` frames on a **copy** of `world` and says what happened, without
/// touching the real world. If no object has `target` attached, a test object is added to the copy,
/// so a script can be tried on its own.
pub fn simulate(
    world: &World,
    scripts: &[(String, String)],
    target: &str,
    ticks: u32,
    dt: f64,
) -> Result<RunReport, Vec<ScriptDiagnostic>> {
    let mut host = ScriptHost::new(world.clone())
        .map_err(|message| vec![ScriptDiagnostic::general("SCRIPT_HOST", message)])?;
    for (name, text) in scripts {
        let result = host.load(name, text);
        if name == target {
            result?;
        }
    }
    if !host.attached().iter().any(|(_, file)| file == target) {
        let record = CommandRecord::new(
            "world.create",
            json!({ "rows": {
                "Name": { "text": "Objeto de teste" },
                "Script": { "file": target, "enabled": true }
            }}),
        );
        let mut state = host.state.0.borrow_mut();
        let GameState {
            world, dispatcher, ..
        } = &mut *state;
        dispatcher.execute(world, &record).map_err(|error| {
            vec![ScriptDiagnostic::general(
                "SCRIPT_TEST_OBJECT",
                error.to_string(),
            )]
        })?;
        state.commands_run = 0;
    }
    let before = host.world().clone();
    for _ in 0..ticks {
        host.update(dt);
    }
    let after = host.world().clone();
    let (created, removed, changed) = diff(&before, &after);
    Ok(RunReport {
        ticks,
        commands: host.commands_run(),
        created,
        removed,
        changed,
        log: host
            .take_log()
            .into_iter()
            .map(|line| (line.level.name().to_string(), line.tick, line.text))
            .collect(),
        faults: host.take_faults(),
    })
}

/// Objects created, removed and changed between two worlds.
fn diff(before: &World, after: &World) -> (usize, usize, usize) {
    let ids = |world: &World| -> BTreeSet<u64> {
        world
            .all_entities()
            .into_iter()
            .filter_map(|handle| world.persistent_id(handle).ok().map(|id| id.0))
            .collect()
    };
    let (old, new) = (ids(before), ids(after));
    let created = new.difference(&old).count();
    let removed = old.difference(&new).count();
    let changed = old
        .intersection(&new)
        .filter(|id| {
            let snapshot = |world: &World| {
                world
                    .handle_of(PersistentId(**id))
                    .ok()
                    .and_then(|handle| world.snapshot(handle).ok())
            };
            snapshot(before).map(|s| s.rows) != snapshot(after).map(|s| s.rows)
        })
        .count();
    (created, removed, changed)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_api_entry_has_a_native() {
        assert!(super::api_is_complete());
    }
}

//! The Script Host: loads scripts, gives each its own VM, and runs their handlers against the
//! world. Everything a script can do is listed in [`crate::api`]; everything it is allowed to do is
//! a capability; and every call runs under an instruction budget, so a script cannot freeze the
//! game.

use std::cell::{Ref, RefCell, RefMut};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::rc::Rc;

use aipo_bytecode::BytecodeModule;
use aipo_host::{Capability, CapabilitySet};
use aipo_sema::PreludeSurface;
use aipo_vm::{AipoEngine, HostNativeCallback, Value, VmError, VmFault};
use petunia_commands::{CommandRecord, Dispatcher};
use petunia_world::{PersistentId, World};
use serde_json::json;

use crate::api::{API, CONTEXT_GLOBAL, GAME_CAPABILITIES, install_standard, script_surface};
use crate::compile::{MAX_SCRIPT_BYTES, ScriptDiagnostic, compile};
use crate::natives::{self, GameCell, GameState, LogLevel, LogLine};
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

struct LoadedScript {
    engine: AipoEngine,
    module: BytecodeModule,
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
        ("transform", "get_position") => natives::transform_get_position,
        ("transform", "set_position") => natives::transform_set_position,
        ("transform", "move") => natives::transform_move,
        ("transform", "move_x") => natives::transform_move_x,
        ("transform", "move_y") => natives::transform_move_y,
        ("transform", "move_z") => natives::transform_move_z,
        ("transform", "get_rotation") => natives::transform_get_rotation,
        ("transform", "set_rotation") => natives::transform_set_rotation,
        ("transform", "rotate") => natives::transform_rotate,
        ("transform", "rotate_x") => natives::transform_rotate_x,
        ("transform", "rotate_y") => natives::transform_rotate_y,
        ("transform", "rotate_z") => natives::transform_rotate_z,
        ("transform", "get_scale") => natives::transform_get_scale,
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
        ("collision", "list") => physics_api::collision_list,
        ("collision", "get") => physics_api::collision_get,
        ("collision", "set") => physics_api::collision_set,
        ("collision", "overlaps") => physics_api::collision_overlaps,
        ("physics", "raycast") => physics_api::physics_raycast,
        ("physics", "velocity") => physics_api::physics_velocity,
        ("physics", "set_velocity") => physics_api::physics_set_velocity,
        ("physics", "impulse") => physics_api::physics_impulse,
        _ => return None,
    })
}

/// Every API entry has a native (checked by a test).
#[cfg(test)]
pub(crate) fn api_is_complete() -> bool {
    API.iter()
        .all(|function| callback_for(function.module, function.name).is_some())
}

fn install_api(engine: &mut AipoEngine) {
    let mut modules: BTreeMap<&str, Vec<(Value, Value)>> = BTreeMap::new();
    for function in API {
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
    scripts: BTreeMap<String, LoadedScript>,
    started: BTreeSet<u64>,
    disabled: BTreeSet<u64>,
    faults: Vec<ScriptFault>,
    limits: ScriptLimits,
}

impl ScriptHost {
    /// A host that owns `world`; scripts change it through the command dispatcher.
    pub fn new(world: World) -> Result<Self, String> {
        let dispatcher = Dispatcher::with_builtin().map_err(|error| error.to_string())?;
        Ok(Self {
            state: GameCell(Rc::new(RefCell::new(GameState {
                world,
                dispatcher,
                seconds: 0.0,
                commands_run: 0,
                log: Vec::new(),
                events: Vec::new(),
                physics: None,
            }))),
            surface: script_surface(),
            scripts: BTreeMap::new(),
            started: BTreeSet::new(),
            disabled: BTreeSet::new(),
            faults: Vec::new(),
            limits: ScriptLimits::default(),
        })
    }

    pub fn set_limits(&mut self, limits: ScriptLimits) {
        self.limits = limits;
    }

    pub fn limits(&self) -> ScriptLimits {
        self.limits
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
    /// `RigidBody` tables. `hulls` gives the points of `convex` colliders on objects without a
    /// `Mesh` (the caller builds them from the brush shape). Returns the problems: colliders and
    /// bodies left out, each naming the object; they are also written to the console.
    pub fn attach_physics(
        &mut self,
        settings: petunia_physics::PhysicsSettings,
        hulls: BTreeMap<u64, Vec<petunia_math::Vec3>>,
    ) -> Result<Vec<String>, String> {
        let mut state = self.state.0.borrow_mut();
        let (runtime, problems) = PhysicsRuntime::new(&state.world, settings, hulls)?;
        state.physics = Some(runtime);
        for problem in &problems {
            state.push_log(LogLevel::Warning, format!("física: {problem}"));
        }
        Ok(problems)
    }

    pub fn has_physics(&self) -> bool {
        self.state.0.borrow().physics.is_some()
    }

    pub fn loaded_scripts(&self) -> Vec<String> {
        self.scripts.keys().cloned().collect()
    }

    /// Gives the world back (the scripts are dropped first, so nothing else holds it).
    pub fn into_world(self) -> World {
        let Self { state, scripts, .. } = self;
        drop(scripts);
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
        let mut engine = self
            .build_engine()
            .map_err(|message| vec![ScriptDiagnostic::general("SCRIPT_HOST", message)])?;
        engine.register_module_declarations(&module);
        engine.reset_instruction_count();
        if let Err(error) = engine.run(&module) {
            let mut diagnostics = compiled.diagnostics;
            let (code, message) = describe(&error, self.limits.instructions_per_call);
            diagnostics.push(ScriptDiagnostic::general(
                &code,
                format!("ao carregar: {message}"),
            ));
            return Err(diagnostics);
        }
        self.scripts
            .insert(name.to_string(), LoadedScript { engine, module });
        self.faults.retain(|fault| fault.script != name);
        let attached = self.attached();
        for (entity, file) in attached {
            if file == name {
                self.disabled.remove(&entity);
            }
        }
        Ok(compiled.diagnostics)
    }

    /// Loads every `.aipo` file of `directory` (not recursive). A file that does not compile is
    /// reported and skipped; the rest still load.
    pub fn load_directory(&mut self, directory: &Path) -> LoadReport {
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

    /// Objects that have a script, as `(entity, script name)`, in id order. A script that is
    /// switched off in the `Script` row is left out.
    pub fn attached(&self) -> Vec<(u64, String)> {
        use petunia_world::Value as WorldValue;
        let state = self.state.0.borrow();
        let Ok(handles) = state.world.entities_with_table("Script") else {
            return Vec::new();
        };
        let mut attached: Vec<(u64, String)> = handles
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
                (enabled && !file.is_empty()).then(|| (id, file.clone()))
            })
            .collect();
        attached.sort();
        attached
    }

    /// Calls `function` of a script. `Ok(None)` when the script does not define it.
    fn call(
        &mut self,
        script: &str,
        function: &str,
        arguments: &[Value],
    ) -> Result<Option<Value>, (String, String)> {
        let limit = self.limits.instructions_per_call;
        let Some(loaded) = self.scripts.get_mut(script) else {
            return Err((
                "SCRIPT_NOT_LOADED".to_string(),
                format!("o script `{script}` não está carregado"),
            ));
        };
        let Some(callee) = loaded.engine.vm().get_global(function).cloned() else {
            return Ok(None);
        };
        if !matches!(callee, Value::Function { .. } | Value::Closure(_)) {
            return Ok(None);
        }
        loaded.engine.set_instruction_budget(Some(limit));
        loaded.engine.reset_instruction_count();
        match loaded
            .engine
            .vm_mut()
            .invoke(&loaded.module, callee, arguments)
        {
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
        self.disabled.insert(entity);
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
        if let Err((code, message)) = self.call(script, function, &arguments) {
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
            if self.disabled.contains(&entity) {
                continue;
            }
            if self.started.insert(entity) {
                self.call_handler(entity, &script, "on_start", &[]);
                if self.disabled.contains(&entity) {
                    continue;
                }
            }
            self.call_handler(entity, &script, "on_update", &[Value::Float(dt)]);
        }
        let events = std::mem::take(&mut self.state.0.borrow_mut().events);
        let attached: BTreeMap<u64, String> = self.attached().into_iter().collect();
        for event in events {
            if self.disabled.contains(&event.entity) {
                continue;
            }
            if let Some(script) = attached.get(&event.entity).cloned() {
                self.call_handler(
                    event.entity,
                    &script,
                    "on_event",
                    &[Value::string(event.name)],
                );
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
        let attached: BTreeMap<u64, String> = self.attached().into_iter().collect();
        for contact in contacts {
            let handler = match contact.phase {
                petunia_physics::ContactPhase::Started => "on_collision",
                petunia_physics::ContactPhase::Stopped => "on_collision_end",
            };
            for (mine, theirs) in [(&contact.first, &contact.second), (&contact.second, &contact.first)] {
                if mine.owner == theirs.owner || self.disabled.contains(&mine.owner) {
                    continue;
                }
                if let Some(script) = attached.get(&mine.owner).cloned() {
                    self.call_handler(
                        mine.owner,
                        &script,
                        handler,
                        &[Value::Int(theirs.owner as i64), Value::string(mine.name.clone()), Value::string(theirs.name.clone())],
                    );
                }
            }
        }
    }

    /// Sends an event to one object from the host (a key, a trigger).
    pub fn send_event(&mut self, entity: u64, name: &str) {
        if self.disabled.contains(&entity) {
            return;
        }
        let script = self
            .attached()
            .into_iter()
            .find(|(id, _)| *id == entity)
            .map(|(_, script)| script);
        if let Some(script) = script {
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

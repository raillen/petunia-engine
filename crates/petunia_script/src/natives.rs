//! The native functions behind the script API. They reach the game through a hidden global that
//! holds a generational handle to the shared state, so there is no process-wide variable: two hosts
//! in one process cannot see each other. Every world change goes through the command dispatcher, so
//! it is validated like a change from the editor, a macro or the network.

use std::cell::RefCell;
use std::rc::Rc;

use aipo_host::Capability;
use aipo_vm::{Value, Vm, VmError, VmFault};
use petunia_commands::{CommandRecord, Dispatcher};
use petunia_math::{Transform, Vec3};
use petunia_world::{PersistentId, World};
use serde_json::{Value as Json, json};

use crate::api::CONTEXT_GLOBAL;
use crate::convert::{display_text, value_to_json, vec3_from_script, vec3_value, world_to_script};

/// The console keeps this many lines; older ones are dropped.
const MAX_LOG_LINES: usize = 1000;
/// A script queues at most this many events per call.
pub(crate) const MAX_QUEUED_EVENTS: usize = 256;
/// A frame keeps this many debug marks; older ones are dropped (the newest frame wins).
const MAX_DEBUG_EVENTS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl LogLevel {
    pub fn name(self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LogLine {
    pub level: LogLevel,
    pub tick: u64,
    pub text: String,
}

/// An event a script asked for with `game.emit`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueuedEvent {
    pub name: String,
    pub entity: u64,
}

/// What scripts share: the world, the dispatcher that changes it, and what they produced.
pub struct GameState {
    pub world: World,
    pub dispatcher: Dispatcher,
    pub seconds: f64,
    pub commands_run: u64,
    pub log: Vec<LogLine>,
    pub events: Vec<QueuedEvent>,
    /// What the generated graph code reported this frame (play mode, N5).
    pub debug: DebugTrace,
    /// The physics of the game, when the host was given one (`ScriptHost::attach_physics`).
    pub physics: Option<crate::physics_api::PhysicsRuntime>,
}

/// One thing the generated code of a graph reported (N5).
#[derive(Clone, Debug, PartialEq)]
pub enum DebugEvent {
    /// This node started running.
    Ran { entity: u64, node: u32 },
    /// This `Se` decided: true or false.
    Check { entity: u64, node: u32, value: bool },
    /// A value the condition was made of, already written as text.
    Value {
        entity: u64,
        node: u32,
        text: String,
    },
}

/// The marks of one or more frames, each with the frame it belongs to.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebugTrace {
    pub events: Vec<(u64, DebugEvent)>,
}

impl GameState {
    pub fn push_log(&mut self, level: LogLevel, text: String) {
        if self.log.len() >= MAX_LOG_LINES {
            self.log.remove(0);
        }
        self.log.push(LogLine {
            level,
            tick: self.world.tick(),
            text,
        });
    }

    fn debug_push(&mut self, event: DebugEvent) {
        let tick = self.world.tick();
        if let Some((last_tick, last)) = self.debug.events.last()
            && *last_tick == tick
            && matches!((last, &event), (DebugEvent::Ran { entity: a, node: b }, DebugEvent::Ran { entity: c, node: d }) if a == c && b == d)
        {
            return;
        }
        if self.debug.events.len() >= MAX_DEBUG_EVENTS {
            self.debug.events.remove(0);
        }
        self.debug.events.push((tick, event));
    }

    /// What the newest frame reported, so a highlight means "ran this frame".
    pub fn debug_latest(&self) -> (u64, Vec<DebugEvent>) {
        let tick = self.debug.events.last().map(|(tick, _)| *tick).unwrap_or(0);
        let events = self
            .debug
            .events
            .iter()
            .filter(|(event_tick, _)| *event_tick == tick)
            .map(|(_, event)| event.clone())
            .collect();
        (tick, events)
    }
}

/// The shared handle to the game state, stored in the VM as userdata.
#[derive(Clone)]
pub(crate) struct GameCell(pub Rc<RefCell<GameState>>);

pub(crate) type Native = Result<Value, VmError>;

pub(crate) fn internal(reason: &str) -> VmError {
    VmError::Fault(VmFault::CorruptedBytecode {
        offset: 0,
        reason: reason.to_string(),
    })
}

pub(crate) fn wrong_type(expected: &str, actual: &Value) -> VmError {
    VmError::Fault(VmFault::TypeMismatch {
        expected: expected.to_string(),
        actual: actual.type_name().to_string(),
    })
}

/// The game state, after checking the capability the call needs (`""` for none).
pub(crate) fn context(vm: &mut Vm, permission: &str, operation: &str) -> Result<GameCell, VmError> {
    let handle = match vm.get_global(CONTEXT_GLOBAL) {
        Some(Value::HostHandle(handle)) => *handle,
        _ => return Err(internal("o contexto do jogo não está disponível")),
    };
    if !permission.is_empty() {
        let capability =
            Capability::parse(permission).map_err(|_| internal("capability inválida"))?;
        vm.host()
            .require(&capability, operation)
            .map_err(VmError::Fault)?;
    }
    vm.host()
        .resolve_userdata::<GameCell>(handle)
        .cloned()
        .map_err(VmError::Fault)
}

pub(crate) fn entity_arg(args: &[Value], index: usize, operation: &str) -> Result<u64, VmError> {
    match args.get(index) {
        Some(Value::Int(id)) if *id >= 0 => Ok(*id as u64),
        Some(other) => Err(wrong_type(
            &format!("Int (id de objeto) em {operation}"),
            other,
        )),
        None => Err(internal("faltou o argumento")),
    }
}

pub(crate) fn node_arg(args: &[Value], index: usize, operation: &str) -> Result<u32, VmError> {
    match args.get(index) {
        Some(Value::Int(id)) if *id >= 0 && *id <= i64::from(u32::MAX) => Ok(*id as u32),
        Some(other) => Err(wrong_type(&format!("Int (id de nó) em {operation}"), other)),
        None => Err(internal("faltou o argumento")),
    }
}

pub(crate) fn bool_arg(args: &[Value], index: usize, operation: &str) -> Result<bool, VmError> {
    match args.get(index) {
        Some(Value::Bool(value)) => Ok(*value),
        Some(other) => Err(wrong_type(&format!("Bool em {operation}"), other)),
        None => Err(internal("faltou o argumento")),
    }
}

pub(crate) fn text_arg(args: &[Value], index: usize, operation: &str) -> Result<String, VmError> {
    match args.get(index) {
        Some(Value::String(text)) => Ok(text.to_string()),
        Some(other) => Err(wrong_type(&format!("String em {operation}"), other)),
        None => Err(internal("faltou o argumento")),
    }
}

pub(crate) fn float_arg(args: &[Value], index: usize, operation: &str) -> Result<f32, VmError> {
    match args.get(index) {
        Some(Value::Float(real)) if real.is_finite() => {
            let cast = *real as f32;
            if cast.is_finite() {
                Ok(cast)
            } else {
                Err(wrong_type(
                    &format!("Float finito em {operation}"),
                    &Value::Float(*real),
                ))
            }
        }
        Some(Value::Int(whole)) => Ok(*whole as f32),
        Some(Value::Byte(byte)) => Ok(f32::from(*byte)),
        Some(other) => Err(wrong_type(&format!("Float em {operation}"), other)),
        None => Err(internal("faltou o argumento")),
    }
}

pub(crate) fn vec3_to_value(vector: Vec3) -> Value {
    vec3_value(vector)
}

pub(crate) fn read_transform(world: &World, entity: u64) -> Result<Transform, String> {
    let handle = world
        .handle_of(PersistentId(entity))
        .map_err(|_| format!("o objeto {entity} não existe"))?;
    let position = match world.get(handle, "Transform", "position") {
        Ok(petunia_world::Value::Vec3(v)) => *v,
        Ok(_) => return Err(format!("a coluna `position` de {entity} não é um Vec3")),
        Err(err) => return Err(err.to_string()),
    };
    let rotation_degrees = match world.get(handle, "Transform", "rotation_degrees") {
        Ok(petunia_world::Value::Vec3(v)) => *v,
        Ok(_) => {
            return Err(format!(
                "a coluna `rotation_degrees` de {entity} não é um Vec3"
            ));
        }
        Err(err) => return Err(err.to_string()),
    };
    let scale = match world.get(handle, "Transform", "scale") {
        Ok(petunia_world::Value::Vec3(v)) => *v,
        Ok(_) => return Err(format!("a coluna `scale` de {entity} não é um Vec3")),
        Err(err) => return Err(err.to_string()),
    };
    Ok(Transform {
        position,
        rotation_degrees,
        scale,
    })
}

pub(crate) fn failure(message: impl Into<String>) -> Native {
    Ok(Value::failure(message))
}

/// Runs a command through the dispatcher; a refusal is a recoverable `Failure` the script can
/// handle, not a crash.
pub(crate) fn run_command(cell: &GameCell, command: &str, payload: Json) -> Result<Json, String> {
    let mut state = cell.0.borrow_mut();
    let GameState {
        world,
        dispatcher,
        commands_run,
        ..
    } = &mut *state;
    *commands_run += 1;
    let touches_physics = crate::physics_api::touches_physics(command, &payload);
    let result = dispatcher
        .execute(world, &CommandRecord::new(command, payload))
        .map_err(|error| error.to_string());
    if result.is_ok() && touches_physics {
        // A collider or a body changed: the simulation is rebuilt before the next step.
        if let Some(runtime) = state.physics.as_mut() {
            runtime.dirty = true;
        }
    }
    result
}

pub(crate) fn world_get(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "world.get")?;
    let entity = entity_arg(args, 0, "world.get")?;
    let table = text_arg(args, 1, "world.get")?;
    let column = text_arg(args, 2, "world.get")?;
    let state = cell.0.borrow();
    let Ok(handle) = state.world.handle_of(PersistentId(entity)) else {
        return failure(format!("o objeto {entity} não existe"));
    };
    match state.world.get(handle, &table, &column) {
        Ok(value) => Ok(world_to_script(&value)),
        Err(error) => failure(error.to_string()),
    }
}

pub(crate) fn world_row(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "world.row")?;
    let entity = entity_arg(args, 0, "world.row")?;
    let table = text_arg(args, 1, "world.row")?;
    let state = cell.0.borrow();
    let Ok(handle) = state.world.handle_of(PersistentId(entity)) else {
        return failure(format!("o objeto {entity} não existe"));
    };
    let snapshot = match state.world.snapshot(handle) {
        Ok(snapshot) => snapshot,
        Err(error) => return failure(error.to_string()),
    };
    let Some(values) = snapshot.rows.get(&table) else {
        return failure(format!("o objeto {entity} não tem a tabela `{table}`"));
    };
    let spec = match state.world.table_spec(&table) {
        Some(spec) => spec,
        None => return failure(format!("a tabela `{table}` não existe")),
    };
    let entries = spec
        .columns
        .iter()
        .zip(values.iter())
        .map(|(column, value)| (Value::string(column.name.clone()), world_to_script(value)))
        .collect();
    Ok(Value::Dict(Rc::new(RefCell::new(
        aipo_vm::DictMap::from_entries(entries),
    ))))
}

pub(crate) fn world_has(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "world.has")?;
    let entity = entity_arg(args, 0, "world.has")?;
    let table = text_arg(args, 1, "world.has")?;
    let state = cell.0.borrow();
    let has = state
        .world
        .handle_of(PersistentId(entity))
        .ok()
        .and_then(|handle| state.world.snapshot(handle).ok())
        .is_some_and(|snapshot| snapshot.rows.contains_key(&table));
    Ok(Value::Bool(has))
}

pub(crate) fn world_exists(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "world.exists")?;
    let entity = entity_arg(args, 0, "world.exists")?;
    let exists = cell
        .0
        .borrow()
        .world
        .handle_of(PersistentId(entity))
        .is_ok();
    Ok(Value::Bool(exists))
}

pub(crate) fn world_find(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "world.find")?;
    let table = text_arg(args, 0, "world.find")?;
    let state = cell.0.borrow();
    let handles = match state.world.entities_with_table(&table) {
        Ok(handles) => handles,
        Err(error) => return failure(error.to_string()),
    };
    let mut ids: Vec<i64> = handles
        .into_iter()
        .filter_map(|handle| state.world.persistent_id(handle).ok())
        .map(|id| id.0 as i64)
        .collect();
    ids.sort_unstable();
    Ok(Value::list(
        ids.into_iter().map(Value::Int).collect::<Vec<_>>(),
    ))
}

pub(crate) fn world_set(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "world.set")?;
    let entity = entity_arg(args, 0, "world.set")?;
    let table = text_arg(args, 1, "world.set")?;
    let column = text_arg(args, 2, "world.set")?;
    let Some(value) = args.get(3) else {
        return Err(internal("faltou o argumento"));
    };
    let value = match value_to_json(value) {
        Ok(json) => json,
        Err(reason) => return failure(reason),
    };
    match run_command(
        &cell,
        "world.set",
        json!({ "entity": entity, "table": table, "column": column, "value": value }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn world_create(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "world.create")?;
    let Some(rows) = args.first() else {
        return Err(internal("faltou o argumento"));
    };
    if !matches!(rows, Value::Dict(_)) {
        return Err(wrong_type("Dict (tabelas) em world.create", rows));
    }
    let rows = match value_to_json(rows) {
        Ok(json) => json,
        Err(reason) => return failure(reason),
    };
    match run_command(&cell, "world.create", json!({ "rows": rows })) {
        Ok(result) => match result.get("entity").and_then(Json::as_i64) {
            Some(id) => Ok(Value::Int(id)),
            None => failure("o motor não devolveu o id do objeto"),
        },
        Err(message) => failure(message),
    }
}

pub(crate) fn world_remove(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "world.remove")?;
    let entity = entity_arg(args, 0, "world.remove")?;
    // An object takes its colliders with it, like deleting it in the editor.
    let parts = crate::physics_api::collider_ids_of(&cell.0.borrow().world, entity);
    for part in parts {
        if let Err(message) = run_command(&cell, "world.remove", json!({ "entity": part })) {
            return failure(message);
        }
    }
    match run_command(&cell, "world.remove", json!({ "entity": entity })) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn game_tick(vm: &mut Vm, _args: &[Value]) -> Native {
    let cell = context(vm, "game", "game.tick")?;
    let tick = cell.0.borrow().world.tick();
    Ok(Value::Int(tick as i64))
}

pub(crate) fn game_seconds(vm: &mut Vm, _args: &[Value]) -> Native {
    let cell = context(vm, "game", "game.seconds")?;
    let seconds = cell.0.borrow().seconds;
    Ok(Value::Float(seconds))
}

pub(crate) fn game_emit(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "game", "game.emit")?;
    let name = text_arg(args, 0, "game.emit")?;
    let entity = entity_arg(args, 1, "game.emit")?;
    let mut state = cell.0.borrow_mut();
    if state.events.len() >= MAX_QUEUED_EVENTS {
        return failure("eventos demais na fila neste quadro");
    }
    state.events.push(QueuedEvent { name, entity });
    Ok(Value::None)
}

fn write_log(vm: &mut Vm, args: &[Value], level: LogLevel) -> Native {
    let cell = context(vm, "", "log")?;
    let text = args.iter().map(display_text).collect::<Vec<_>>().join(" ");
    cell.0.borrow_mut().push_log(level, text);
    Ok(Value::None)
}

/// Longest text a debug value keeps; enough to read a condition, short of a whole table.
const MAX_DEBUG_TEXT: usize = 200;

pub(crate) fn graph_debug_node(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "", "graph_debug.node")?;
    let entity = entity_arg(args, 0, "graph_debug.node")?;
    let node = node_arg(args, 1, "graph_debug.node")?;
    cell.0
        .borrow_mut()
        .debug_push(DebugEvent::Ran { entity, node });
    Ok(Value::None)
}

pub(crate) fn graph_debug_check(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "", "graph_debug.check")?;
    let entity = entity_arg(args, 0, "graph_debug.check")?;
    let node = node_arg(args, 1, "graph_debug.check")?;
    let value = bool_arg(args, 2, "graph_debug.check")?;
    cell.0.borrow_mut().debug_push(DebugEvent::Check {
        entity,
        node,
        value,
    });
    Ok(Value::None)
}

pub(crate) fn graph_debug_value(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "", "graph_debug.value")?;
    let entity = entity_arg(args, 0, "graph_debug.value")?;
    let node = node_arg(args, 1, "graph_debug.value")?;
    let Some(value) = args.get(2) else {
        return Err(internal("faltou o argumento"));
    };
    let mut text = display_text(value);
    text.truncate(MAX_DEBUG_TEXT);
    cell.0
        .borrow_mut()
        .debug_push(DebugEvent::Value { entity, node, text });
    Ok(Value::None)
}

pub(crate) fn log_debug(vm: &mut Vm, args: &[Value]) -> Native {
    write_log(vm, args, LogLevel::Debug)
}
pub(crate) fn log_info(vm: &mut Vm, args: &[Value]) -> Native {
    write_log(vm, args, LogLevel::Info)
}
pub(crate) fn log_warning(vm: &mut Vm, args: &[Value]) -> Native {
    write_log(vm, args, LogLevel::Warning)
}
pub(crate) fn log_error(vm: &mut Vm, args: &[Value]) -> Native {
    write_log(vm, args, LogLevel::Error)
}

// ---- transform.* -----------------------------------------------------------------------------

fn vec3_arg(args: &[Value], index: usize, operation: &str) -> Result<Vec3, VmError> {
    let Some(value) = args.get(index) else {
        return Err(internal("faltou o argumento"));
    };
    vec3_from_script(value).map_err(|_| wrong_type(&format!("Vec3 em {operation}"), value))
}

pub(crate) fn transform_get_position(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "transform.position")?;
    let entity = entity_arg(args, 0, "transform.position")?;
    let state = cell.0.borrow();
    let Ok(handle) = state.world.handle_of(PersistentId(entity)) else {
        return failure(format!("o objeto {entity} não existe"));
    };
    match state.world.get(handle, "Transform", "position") {
        Ok(petunia_world::Value::Vec3(v)) => Ok(vec3_to_value(*v)),
        Ok(_) => failure(format!("a coluna `position` de {entity} não é um Vec3")),
        Err(err) => failure(err.to_string()),
    }
}

pub(crate) fn transform_set_position(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.set_position")?;
    let entity = entity_arg(args, 0, "transform.set_position")?;
    let position = vec3_arg(args, 1, "transform.set_position")?;
    let x = position.x;
    let y = position.y;
    let z = position.z;
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "position",
            "value": { "x": x, "y": y, "z": z }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_move(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.move")?;
    let entity = entity_arg(args, 0, "transform.move")?;
    let offset = vec3_arg(args, 1, "transform.move")?;
    let dx = offset.x;
    let dy = offset.y;
    let dz = offset.z;
    let current = {
        let state = cell.0.borrow();
        match read_transform(&state.world, entity) {
            Ok(transform) => transform.position,
            Err(err) => return failure(err),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "position",
            "value": { "x": current.x + dx, "y": current.y + dy, "z": current.z + dz }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_move_x(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.move_x")?;
    let entity = entity_arg(args, 0, "transform.move_x")?;
    let delta = float_arg(args, 1, "transform.move_x")?;
    let current = {
        let state = cell.0.borrow();
        match read_transform(&state.world, entity) {
            Ok(transform) => transform.position,
            Err(err) => return failure(err),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "position",
            "value": { "x": current.x + delta, "y": current.y, "z": current.z }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_move_y(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.move_y")?;
    let entity = entity_arg(args, 0, "transform.move_y")?;
    let delta = float_arg(args, 1, "transform.move_y")?;
    let current = {
        let state = cell.0.borrow();
        match read_transform(&state.world, entity) {
            Ok(transform) => transform.position,
            Err(err) => return failure(err),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "position",
            "value": { "x": current.x, "y": current.y + delta, "z": current.z }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_move_z(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.move_z")?;
    let entity = entity_arg(args, 0, "transform.move_z")?;
    let delta = float_arg(args, 1, "transform.move_z")?;
    let current = {
        let state = cell.0.borrow();
        match read_transform(&state.world, entity) {
            Ok(transform) => transform.position,
            Err(err) => return failure(err),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "position",
            "value": { "x": current.x, "y": current.y, "z": current.z + delta }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_get_rotation(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "transform.rotation")?;
    let entity = entity_arg(args, 0, "transform.rotation")?;
    let state = cell.0.borrow();
    let Ok(handle) = state.world.handle_of(PersistentId(entity)) else {
        return failure(format!("o objeto {entity} não existe"));
    };
    match state.world.get(handle, "Transform", "rotation_degrees") {
        Ok(petunia_world::Value::Vec3(v)) => Ok(vec3_to_value(*v)),
        Ok(_) => failure(format!(
            "a coluna `rotation_degrees` de {entity} não é um Vec3"
        )),
        Err(err) => failure(err.to_string()),
    }
}

pub(crate) fn transform_set_rotation(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.set_rotation")?;
    let entity = entity_arg(args, 0, "transform.set_rotation")?;
    let rotation = vec3_arg(args, 1, "transform.set_rotation")?;
    let x = rotation.x;
    let y = rotation.y;
    let z = rotation.z;
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "rotation_degrees",
            "value": { "x": x, "y": y, "z": z }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_rotate(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.rotate")?;
    let entity = entity_arg(args, 0, "transform.rotate")?;
    let degrees = vec3_arg(args, 1, "transform.rotate")?;
    let dx = degrees.x;
    let dy = degrees.y;
    let dz = degrees.z;
    let current = {
        let state = cell.0.borrow();
        match read_transform(&state.world, entity) {
            Ok(transform) => transform.rotation_degrees,
            Err(err) => return failure(err),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "rotation_degrees",
            "value": { "x": current.x + dx, "y": current.y + dy, "z": current.z + dz }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_rotate_x(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.rotate_x")?;
    let entity = entity_arg(args, 0, "transform.rotate_x")?;
    let degrees = float_arg(args, 1, "transform.rotate_x")?;
    let current = {
        let state = cell.0.borrow();
        match read_transform(&state.world, entity) {
            Ok(transform) => transform.rotation_degrees,
            Err(err) => return failure(err),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "rotation_degrees",
            "value": { "x": current.x + degrees, "y": current.y, "z": current.z }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_rotate_y(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.rotate_y")?;
    let entity = entity_arg(args, 0, "transform.rotate_y")?;
    let degrees = float_arg(args, 1, "transform.rotate_y")?;
    let current = {
        let state = cell.0.borrow();
        match read_transform(&state.world, entity) {
            Ok(transform) => transform.rotation_degrees,
            Err(err) => return failure(err),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "rotation_degrees",
            "value": { "x": current.x, "y": current.y + degrees, "z": current.z }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_rotate_z(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.rotate_z")?;
    let entity = entity_arg(args, 0, "transform.rotate_z")?;
    let degrees = float_arg(args, 1, "transform.rotate_z")?;
    let current = {
        let state = cell.0.borrow();
        match read_transform(&state.world, entity) {
            Ok(transform) => transform.rotation_degrees,
            Err(err) => return failure(err),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "rotation_degrees",
            "value": { "x": current.x, "y": current.y, "z": current.z + degrees }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_get_scale(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "transform.scale")?;
    let entity = entity_arg(args, 0, "transform.scale")?;
    let state = cell.0.borrow();
    let Ok(handle) = state.world.handle_of(PersistentId(entity)) else {
        return failure(format!("o objeto {entity} não existe"));
    };
    match state.world.get(handle, "Transform", "scale") {
        Ok(petunia_world::Value::Vec3(v)) => Ok(vec3_to_value(*v)),
        Ok(_) => failure(format!("a coluna `scale` de {entity} não é um Vec3")),
        Err(err) => failure(err.to_string()),
    }
}

pub(crate) fn transform_set_scale(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "transform.set_scale")?;
    let entity = entity_arg(args, 0, "transform.set_scale")?;
    let scale = vec3_arg(args, 1, "transform.set_scale")?;
    let x = scale.x;
    let y = scale.y;
    let z = scale.z;
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Transform",
            "column": "scale",
            "value": { "x": x, "y": y, "z": z }
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn transform_forward(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "transform.forward")?;
    let entity = entity_arg(args, 0, "transform.forward")?;
    let state = cell.0.borrow();
    match read_transform(&state.world, entity) {
        Ok(transform) => Ok(vec3_to_value(transform.forward())),
        Err(err) => failure(err),
    }
}

pub(crate) fn transform_right(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "transform.right")?;
    let entity = entity_arg(args, 0, "transform.right")?;
    let state = cell.0.borrow();
    match read_transform(&state.world, entity) {
        Ok(transform) => Ok(vec3_to_value(transform.right())),
        Err(err) => failure(err),
    }
}

pub(crate) fn transform_up(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "transform.up")?;
    let entity = entity_arg(args, 0, "transform.up")?;
    let state = cell.0.borrow();
    match read_transform(&state.world, entity) {
        Ok(transform) => Ok(vec3_to_value(transform.up())),
        Err(err) => failure(err),
    }
}

pub(crate) fn interact_prompt(vm: &mut Vm, _args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "interact.prompt")?;
    let offer = crate::gameplay::nearest_interact(&cell.0.borrow().world);
    Ok(match offer {
        Some(offer) => Value::string(offer.prompt),
        None => Value::None,
    })
}

pub(crate) fn health_current(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "health.current")?;
    let entity = entity_arg(args, 0, "health.current")?;
    let state = cell.0.borrow();
    match health_pair(&state.world, entity) {
        Ok((current, _)) => Ok(Value::Float(current)),
        Err(message) => failure(message),
    }
}

pub(crate) fn health_damage(vm: &mut Vm, args: &[Value]) -> Native {
    health_change(vm, args, "health.damage", -1.0)
}

pub(crate) fn health_heal(vm: &mut Vm, args: &[Value]) -> Native {
    health_change(vm, args, "health.heal", 1.0)
}

fn health_change(vm: &mut Vm, args: &[Value], operation: &str, sign: f64) -> Native {
    let cell = context(vm, "world.write", operation)?;
    let entity = entity_arg(args, 0, operation)?;
    let amount = f64::from(float_arg(args, 1, operation)?);
    if amount < 0.0 {
        return failure(format!("{operation} pede um número maior ou igual a zero"));
    }
    let (current, maximum) = {
        let state = cell.0.borrow();
        match health_pair(&state.world, entity) {
            Ok(pair) => pair,
            Err(message) => return failure(message),
        }
    };
    let ceiling = if maximum.is_finite() && maximum > 0.0 {
        maximum
    } else {
        current.max(0.0)
    };
    let next = (current + sign * amount).clamp(0.0, ceiling);
    match run_command(
        &cell,
        "world.set",
        json!({
            "entity": entity,
            "table": "Health",
            "column": "current",
            "value": next
        }),
    ) {
        Ok(_) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

fn health_pair(world: &World, entity: u64) -> Result<(f64, f64), String> {
    let handle = world
        .handle_of(PersistentId(entity))
        .map_err(|_| format!("o objeto {entity} não existe"))?;
    let number = |column: &str| match world.get(handle, "Health", column) {
        Ok(petunia_world::Value::Float(value)) if value.is_finite() => Ok(*value),
        Ok(_) => Err(format!("a coluna `{column}` de {entity} não é um número")),
        Err(error) => Err(error.to_string()),
    };
    Ok((number("current")?, number("maximum")?))
}

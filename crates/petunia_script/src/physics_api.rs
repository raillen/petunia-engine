//! Collision and physics for scripts: the named colliders of an object (`collision.*`), the
//! running simulation (`physics.*`) and the contacts that call `on_collision` and
//! `on_collision_end`. The simulation itself lives in `petunia_physics`; this module only reads
//! and writes the game state and turns values into what scripts see.

use std::collections::BTreeMap;

use aipo_vm::{Value, Vm};
use petunia_math::{Transform, Vec3};
use petunia_physics::{BodyKind, ContactEvent, PhysicsSettings, Simulation, SolidShape, describe};
use petunia_world::{PersistentId, World};
use serde_json::json;

use crate::convert::{json_to_value, value_to_json, vec3_value};
use crate::natives::{
    GameCell, GameState, Native, context, entity_arg, failure, float_arg, internal, read_transform,
    run_command, text_arg, vec3_to_value, wrong_type,
};

/// Most physics steps in one frame; a long pause does not make the game run ahead in a burst.
const MAX_STEPS_PER_FRAME: u32 = 8;
/// The longest frame time the physics catches up with, in seconds.
const MAX_FRAME_SECONDS: f64 = 0.25;

/// The simulation of a running game and what the host needs to keep it in step with the world.
pub struct PhysicsRuntime {
    pub simulation: Simulation,
    settings: PhysicsSettings,
    /// Hull points of objects without a `Mesh` (brushes), kept to rebuild after a change.
    solids: BTreeMap<u64, SolidShape>,
    /// Seconds not yet simulated.
    accumulator: f64,
    /// The pose this runtime last wrote (or applied) per moving body: a different value in the
    /// world means a script moved the object, so the body follows it.
    written: BTreeMap<u64, (Vec3, Vec3)>,
    kinds: BTreeMap<u64, BodyKind>,
    /// Contacts of the frame, delivered after `on_update`.
    pub contacts: Vec<ContactEvent>,
    /// A collider changed (`collision.set`): rebuild before the next step.
    pub dirty: bool,
}

/// What the person asks of the player's character this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerInput {
    /// -1 to 1: backward to forward, relative to `yaw_degrees`.
    pub forward: f32,
    /// -1 to 1: left to right.
    pub right: f32,
    pub jump: bool,
    /// True on the frame the person asks to use whatever is in front of them.
    pub interact: bool,
    /// The direction the camera looks, in degrees around the vertical axis.
    pub yaw_degrees: f32,
}

impl PhysicsRuntime {
    /// Walks every `Character` with `player` set: speed from its table, direction from the camera.
    /// The character turns to face where it goes.
    pub fn drive_players(&mut self, world: &mut World, input: PlayerInput) {
        let finite = |value: f32| {
            if value.is_finite() {
                value.clamp(-1.0, 1.0)
            } else {
                0.0
            }
        };
        let (forward, right) = (finite(input.forward), finite(input.right));
        let heading = Transform {
            rotation_degrees: Vec3::new(
                0.0,
                if input.yaw_degrees.is_finite() {
                    input.yaw_degrees
                } else {
                    0.0
                },
                0.0,
            ),
            ..Transform::IDENTITY
        };
        let mut direction = heading.forward() * forward + heading.right() * right;
        if direction.length() > 1.0 {
            direction = direction.normalized_or_zero();
        }
        for handle in world.entities_with_table("Character").unwrap_or_default() {
            if !matches!(
                world.get(handle, "Character", "player"),
                Ok(petunia_world::Value::Bool(true))
            ) {
                continue;
            }
            let Ok(PersistentId(id)) = world.persistent_id(handle) else {
                continue;
            };
            let speed = match world.get(handle, "Character", "speed") {
                Ok(petunia_world::Value::Float(speed)) => *speed as f32,
                _ => 0.0,
            };
            let velocity = direction * speed.clamp(0.0, 100.0);
            if self.simulation.set_character_intent(id, velocity).is_err() {
                continue;
            }
            if input.jump {
                let _ = self.simulation.character_jump(id);
            }
            if direction.length() > 0.1 {
                let yaw = (-direction.x).atan2(-direction.z).to_degrees();
                if let Ok(petunia_world::Value::Vec3(current)) =
                    world.get(handle, "Transform", "rotation_degrees")
                {
                    let turned = Vec3::new(current.x, yaw, current.z);
                    let _ = world.set(
                        handle,
                        "Transform",
                        "rotation_degrees",
                        petunia_world::Value::Vec3(turned),
                    );
                }
            }
        }
    }

    /// Builds the simulation from the world's `Collider` and `RigidBody` tables. Returns the
    /// runtime and the problems (colliders and bodies that were left out).
    pub fn new(
        world: &World,
        settings: PhysicsSettings,
        solids: BTreeMap<u64, SolidShape>,
    ) -> Result<(Self, Vec<String>), String> {
        let scene = describe(world, &solids);
        let simulation = scene.build(settings).map_err(|error| error.to_string())?;
        let kinds = scene
            .bodies
            .iter()
            .map(|body| (body.entity, body.kind))
            .collect();
        let written = scene
            .bodies
            .iter()
            .filter(|body| body.kind != BodyKind::Static)
            .map(|body| {
                (
                    body.entity,
                    (body.transform.position, body.transform.rotation_degrees),
                )
            })
            .collect();
        Ok((
            Self {
                simulation,
                settings,
                solids,
                accumulator: 0.0,
                written,
                kinds,
                contacts: Vec::new(),
                dirty: false,
            },
            scene.problems,
        ))
    }

    /// Rebuilds after colliders changed, keeping the velocity of every body that still exists.
    fn rebuild(&mut self, world: &World) -> Vec<String> {
        let velocities: Vec<(u64, Vec3)> = self
            .kinds
            .iter()
            .filter(|(_, kind)| **kind == BodyKind::Dynamic)
            .filter_map(|(entity, _)| Some((*entity, self.simulation.velocity(*entity).ok()?)))
            .collect();
        match Self::new(world, self.settings, std::mem::take(&mut self.solids)) {
            Ok((mut fresh, problems)) => {
                for (entity, velocity) in velocities {
                    let _ = fresh.simulation.set_velocity(entity, velocity);
                }
                fresh.accumulator = self.accumulator;
                *self = fresh;
                problems
            }
            Err(message) => {
                self.dirty = false;
                vec![message]
            }
        }
    }

    /// One frame: follow what scripts moved, advance in fixed steps, write the moving bodies back
    /// into their `Transform` (a per-frame update, not a command), and keep the contacts.
    pub fn advance(&mut self, world: &mut World, seconds: f64) -> Vec<String> {
        let mut problems = Vec::new();
        if self.dirty {
            problems = self.rebuild(world);
        }
        for (entity, kind) in self.kinds.clone() {
            if kind == BodyKind::Static {
                continue;
            }
            let Ok(handle) = world.handle_of(PersistentId(entity)) else {
                continue;
            };
            let Ok(transform) = read_transform(world, entity) else {
                continue;
            };
            let current = (transform.position, transform.rotation_degrees);
            if self.written.get(&entity) != Some(&current)
                && world.is_alive(handle)
                && self.simulation.move_body(entity, &transform).is_ok()
            {
                self.written.insert(entity, current);
            }
        }
        self.accumulator += seconds.clamp(0.0, MAX_FRAME_SECONDS);
        let step = f64::from(self.settings.step_seconds);
        let mut steps = 0;
        while self.accumulator >= step && steps < MAX_STEPS_PER_FRAME {
            self.contacts.extend(self.simulation.step());
            self.accumulator -= step;
            steps += 1;
        }
        if steps == MAX_STEPS_PER_FRAME {
            self.accumulator = 0.0;
        }
        for pose in self.simulation.poses() {
            let moves_itself = self.kinds.get(&pose.entity) == Some(&BodyKind::Dynamic)
                || self.simulation.is_character(pose.entity);
            if !moves_itself {
                continue;
            }
            let Ok(handle) = world.handle_of(PersistentId(pose.entity)) else {
                continue;
            };
            let _ = world.set(
                handle,
                "Transform",
                "position",
                petunia_world::Value::Vec3(pose.position),
            );
            let _ = world.set(
                handle,
                "Transform",
                "rotation_degrees",
                petunia_world::Value::Vec3(pose.rotation_degrees),
            );
            self.written
                .insert(pose.entity, (pose.position, pose.rotation_degrees));
        }
        problems
    }
}

// ---- reading the colliders of an object -------------------------------------------------------

/// Whether a command changes colliders or bodies (so the simulation must be rebuilt).
pub(crate) fn touches_physics(command: &str, payload: &serde_json::Value) -> bool {
    let physical = |table: &str| matches!(table, "Collider" | "RigidBody");
    match command {
        "world.set" | "row.set" | "row.create" | "row.remove" => payload
            .get("table")
            .and_then(serde_json::Value::as_str)
            .is_some_and(physical),
        "world.create" => payload
            .get("rows")
            .and_then(serde_json::Value::as_object)
            .is_some_and(|rows| rows.keys().any(|table| physical(table))),
        // Removing an object may remove its body: rebuild to be safe.
        "world.remove" => true,
        _ => false,
    }
}

/// Persistent ids of the colliders of `owner`.
pub(crate) fn collider_ids_of(world: &World, owner: u64) -> Vec<u64> {
    colliders_of(world, owner)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

/// The colliders of `owner`, as (collider entity id, name), in id order.
fn colliders_of(world: &World, owner: u64) -> Vec<(u64, String)> {
    let mut found: Vec<(u64, String)> = world
        .entities_with_table("Collider")
        .unwrap_or_default()
        .into_iter()
        .filter(|handle| matches!(world.get(*handle, "Collider", "owner"), Ok(petunia_world::Value::Int(id)) if *id as u64 == owner))
        .filter_map(|handle| {
            let id = world.persistent_id(handle).ok()?.0;
            let name = match world.get(handle, "Collider", "name") {
                Ok(petunia_world::Value::Text(name)) => name.clone(),
                _ => String::new(),
            };
            Some((id, name))
        })
        .collect();
    found.sort();
    found
}

fn find_collider(world: &World, owner: u64, name: &str) -> Result<u64, String> {
    let colliders = colliders_of(world, owner);
    colliders
        .iter()
        .find(|(_, candidate)| candidate == name)
        .map(|(id, _)| *id)
        .ok_or_else(|| {
            let names: Vec<&str> = colliders.iter().map(|(_, name)| name.as_str()).collect();
            if names.is_empty() {
                format!("o objeto {owner} não tem colisões")
            } else {
                format!(
                    "o objeto {owner} não tem a colisão `{name}` (tem: {})",
                    names.join(", ")
                )
            }
        })
}

fn vector_arg(args: &[Value], index: usize, operation: &str) -> Result<Vec3, aipo_vm::VmError> {
    let Some(value) = args.get(index) else {
        return Err(internal("faltou o argumento"));
    };
    let Ok(json) = value_to_json(value) else {
        return Err(wrong_type(
            &format!("Dict {{x, y, z}} em {operation}"),
            value,
        ));
    };
    let component = |axis: &str| json.get(axis).and_then(serde_json::Value::as_f64);
    match (component("x"), component("y"), component("z")) {
        (Some(x), Some(y), Some(z)) if x.is_finite() && y.is_finite() && z.is_finite() => {
            Ok(Vec3::new(x as f32, y as f32, z as f32))
        }
        _ => Err(wrong_type(
            &format!("Dict {{x, y, z}} em {operation}"),
            value,
        )),
    }
}

fn with_physics<T>(
    cell: &GameCell,
    operation: &str,
    work: impl FnOnce(&mut PhysicsRuntime, &World) -> Result<T, String>,
) -> Result<T, String> {
    let mut state = cell.0.borrow_mut();
    let GameState { world, physics, .. } = &mut *state;
    match physics {
        Some(runtime) => work(runtime, world),
        None => Err(format!("{operation}: a física não está ligada neste jogo")),
    }
}

// ---- collision.* --------------------------------------------------------------------------------

pub(crate) fn collision_list(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "collision.list")?;
    let entity = entity_arg(args, 0, "collision.list")?;
    let state = cell.0.borrow();
    if state.world.handle_of(PersistentId(entity)).is_err() {
        return failure(format!("o objeto {entity} não existe"));
    }
    let names = colliders_of(&state.world, entity)
        .into_iter()
        .map(|(_, name)| Value::string(name))
        .collect::<Vec<_>>();
    Ok(Value::list(names))
}

pub(crate) fn collision_get(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "collision.get")?;
    let entity = entity_arg(args, 0, "collision.get")?;
    let name = text_arg(args, 1, "collision.get")?;
    let state = cell.0.borrow();
    let id = match find_collider(&state.world, entity, &name) {
        Ok(id) => id,
        Err(message) => return failure(message),
    };
    let Ok(handle) = state.world.handle_of(PersistentId(id)) else {
        return failure(format!("a colisão {id} não existe"));
    };
    let values = match state.world.row(handle, "Collider") {
        Ok(values) => values,
        Err(error) => return failure(error.to_string()),
    };
    match state.world.row_to_json("Collider", values) {
        Ok(mut row) => {
            row.insert("collider".to_string(), json!(id));
            Ok(json_to_value(&serde_json::Value::Object(row)))
        }
        Err(error) => failure(error.to_string()),
    }
}

/// Changes one column of a named collider through the command dispatcher (validated, like an
/// edit in the inspector). The simulation is rebuilt before the next step.
pub(crate) fn collision_set(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "collision.set")?;
    let entity = entity_arg(args, 0, "collision.set")?;
    let name = text_arg(args, 1, "collision.set")?;
    let column = text_arg(args, 2, "collision.set")?;
    let Some(value) = args.get(3) else {
        return Err(internal("faltou o argumento"));
    };
    if column == "owner" {
        return failure("o dono de uma colisão não muda por script; crie outra colisão");
    }
    let id = {
        let state = cell.0.borrow();
        match find_collider(&state.world, entity, &name) {
            Ok(id) => id,
            Err(message) => return failure(message),
        }
    };
    match run_command(
        &cell,
        "world.set",
        json!({ "entity": id, "table": "Collider", "column": column, "value": match value_to_json(value) {
            Ok(json) => json,
            Err(message) => return failure(message),
        } }),
    ) {
        Ok(_) => {
            if let Some(runtime) = cell.0.borrow_mut().physics.as_mut() {
                runtime.dirty = true;
            }
            Ok(Value::None)
        }
        Err(message) => failure(message),
    }
}

pub(crate) fn collision_overlaps(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "collision.overlaps")?;
    let entity = entity_arg(args, 0, "collision.overlaps")?;
    let name = text_arg(args, 1, "collision.overlaps")?;
    let found = with_physics(&cell, "collision.overlaps", |runtime, world| {
        let id = find_collider(world, entity, &name)?;
        runtime
            .simulation
            .overlapping(id)
            .map_err(|error| error.to_string())
    });
    match found {
        Ok(found) => Ok(json_to_value(&json!(
            found
                .into_iter()
                .map(|other| json!({ "entity": other.owner, "collider": other.name }))
                .collect::<Vec<_>>()
        ))),
        Err(message) => failure(message),
    }
}

// ---- character.* --------------------------------------------------------------------------------

pub(crate) fn character_move(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "character.move")?;
    let entity = entity_arg(args, 0, "character.move")?;
    let x = float_arg(args, 1, "character.move")?;
    let z = float_arg(args, 2, "character.move")?;
    match with_physics(&cell, "character.move", |runtime, _| {
        runtime
            .simulation
            .set_character_intent(entity, Vec3::new(x, 0.0, z))
            .map_err(|error| error.to_string())
    }) {
        Ok(()) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn character_jump(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "character.jump")?;
    let entity = entity_arg(args, 0, "character.jump")?;
    match with_physics(&cell, "character.jump", |runtime, _| {
        runtime
            .simulation
            .character_jump(entity)
            .map_err(|error| error.to_string())
    }) {
        Ok(()) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn character_grounded(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "character.grounded")?;
    let entity = entity_arg(args, 0, "character.grounded")?;
    match with_physics(&cell, "character.grounded", |runtime, _| {
        runtime
            .simulation
            .character_grounded(entity)
            .map_err(|error| error.to_string())
    }) {
        Ok(grounded) => Ok(Value::Bool(grounded)),
        Err(message) => failure(message),
    }
}

// ---- physics.* ----------------------------------------------------------------------------------

pub(crate) fn physics_raycast(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "physics.raycast")?;
    let origin = vector_arg(args, 0, "physics.raycast")?;
    let direction = vector_arg(args, 1, "physics.raycast")?;
    let distance = float_arg(args, 2, "physics.raycast")?;
    let hit = with_physics(&cell, "physics.raycast", |runtime, _| {
        runtime
            .simulation
            .raycast(origin, direction, distance)
            .map_err(|error| error.to_string())
    });
    hit_value(hit)
}

/// A ray from an object's position that ignores the object's own colliders.
pub(crate) fn physics_raycast_from(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "physics.raycast_from")?;
    let entity = entity_arg(args, 0, "physics.raycast_from")?;
    let direction = vector_arg(args, 1, "physics.raycast_from")?;
    let distance = float_arg(args, 2, "physics.raycast_from")?;
    let hit = with_physics(&cell, "physics.raycast_from", |runtime, world| {
        let origin = read_transform(world, entity)?.position;
        runtime
            .simulation
            .raycast_ignoring(origin, direction, distance, Some(entity))
            .map_err(|error| error.to_string())
    });
    hit_value(hit)
}

fn hit_value(hit: Result<Option<petunia_physics::RayHit>, String>) -> Native {
    match hit {
        Ok(Some(hit)) => {
            let entries = vec![
                (
                    Value::string("entity"),
                    Value::Int(i64::try_from(hit.collider.owner).unwrap_or(0)),
                ),
                (
                    Value::string("collider"),
                    Value::string(hit.collider.name.clone()),
                ),
                (Value::string("point"), vec3_value(hit.point)),
                (Value::string("normal"), vec3_value(hit.normal)),
                (
                    Value::string("distance"),
                    Value::Float(f64::from(hit.distance)),
                ),
            ];
            Ok(Value::Dict(std::rc::Rc::new(std::cell::RefCell::new(
                aipo_vm::DictMap::from_entries(entries),
            ))))
        }
        Ok(None) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn physics_velocity(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.read", "physics.velocity")?;
    let entity = entity_arg(args, 0, "physics.velocity")?;
    match with_physics(&cell, "physics.velocity", |runtime, _| {
        runtime
            .simulation
            .velocity(entity)
            .map_err(|error| error.to_string())
    }) {
        Ok(velocity) => Ok(vec3_to_value(velocity)),
        Err(message) => failure(message),
    }
}

pub(crate) fn physics_set_velocity(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "physics.set_velocity")?;
    let entity = entity_arg(args, 0, "physics.set_velocity")?;
    let velocity = vector_arg(args, 1, "physics.set_velocity")?;
    match with_physics(&cell, "physics.set_velocity", |runtime, _| {
        runtime
            .simulation
            .set_velocity(entity, velocity)
            .map_err(|error| error.to_string())
    }) {
        Ok(()) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

pub(crate) fn physics_impulse(vm: &mut Vm, args: &[Value]) -> Native {
    let cell = context(vm, "world.write", "physics.impulse")?;
    let entity = entity_arg(args, 0, "physics.impulse")?;
    let impulse = vector_arg(args, 1, "physics.impulse")?;
    match with_physics(&cell, "physics.impulse", |runtime, _| {
        runtime
            .simulation
            .apply_impulse(entity, impulse)
            .map_err(|error| error.to_string())
    }) {
        Ok(()) => Ok(Value::None),
        Err(message) => failure(message),
    }
}

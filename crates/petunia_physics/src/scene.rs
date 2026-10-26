//! Reads a world's `Collider` and `RigidBody` tables into the descriptions a [`Simulation`] is
//! built from. One broken collider is reported and left out; it does not stop the game.

use std::collections::BTreeMap;

use petunia_math::{SolidShape, Transform, Vec3};
use petunia_world::{EntityHandle, PersistentId, Value, World};

use crate::{
    BodyDescription, BodyKind, CharacterDescription, ColliderDescription, ColliderShape,
    DEFAULT_LAYER, DEFAULT_MASK, MAX_HULL_POINTS, PhysicsSettings, Simulation, check_body,
    check_collider,
};

/// What a world holds for physics, plus what was wrong with it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SceneDescription {
    pub bodies: Vec<BodyDescription>,
    pub colliders: Vec<ColliderDescription>,
    /// Objects that walk (`Character` table): their body is kinematic.
    pub characters: Vec<CharacterDescription>,
    /// One line per collider or body that was left out, naming the object.
    pub problems: Vec<String>,
}

impl SceneDescription {
    pub fn is_empty(&self) -> bool {
        self.colliders.is_empty() && self.bodies.iter().all(|body| body.kind == BodyKind::Static)
    }

    pub fn build(&self, settings: PhysicsSettings) -> Result<Simulation, crate::PhysicsError> {
        let mut simulation = Simulation::build(settings, &self.bodies, &self.colliders)?;
        simulation.add_characters(&self.characters)?;
        Ok(simulation)
    }
}

/// Colliders made by the engine itself (an object with a shape and no collider) have an id with
/// this bit set, so they never clash with an entity id.
pub const AUTO_COLLIDER_FLAG: u64 = 1 << 62;

/// Reads the physics of `world`. `solids` is the drawn surface of each object (the renderer
/// knows how a brush becomes a mesh, this crate does not): a `convex` collider wraps it, and an
/// object that has a surface but no collider becomes **solid by itself**: a box for a box brush, a
/// capsule for a character, a convex hull for something with a dynamic body, an exact surface for
/// everything else. A scene is walkable without setting anything up.
pub fn describe(world: &World, solids: &BTreeMap<u64, SolidShape>) -> SceneDescription {
    let mut scene = SceneDescription::default();
    let mut owners: BTreeMap<u64, ()> = BTreeMap::new();

    for handle in world.entities_with_table("Collider").unwrap_or_default() {
        let Ok(PersistentId(id)) = world.persistent_id(handle) else {
            continue;
        };
        match read_collider(world, handle, id, solids) {
            Ok(collider) => {
                if let Err(error) = check_collider(&collider) {
                    scene.problems.push(format!("colisão #{id}: {error}"));
                    continue;
                }
                owners.insert(collider.owner, ());
                scene.colliders.push(collider);
            }
            Err(problem) => scene.problems.push(format!("colisão #{id}: {problem}")),
        }
    }

    let mut with_body: BTreeMap<u64, ()> = BTreeMap::new();
    for handle in world.entities_with_table("RigidBody").unwrap_or_default() {
        let Ok(PersistentId(id)) = world.persistent_id(handle) else {
            continue;
        };
        match read_body(world, handle, id) {
            Ok(body) => {
                if let Err(error) = check_body(&body) {
                    scene
                        .problems
                        .push(format!("corpo do objeto #{id}: {error}"));
                    continue;
                }
                with_body.insert(id, ());
                scene.bodies.push(body);
            }
            Err(problem) => scene
                .problems
                .push(format!("corpo do objeto #{id}: {problem}")),
        }
    }

    // Solid by themselves: drawn objects without a collider of their own.
    let mut automatic = Vec::new();
    for (entity, solid) in solids {
        if owners.contains_key(entity) {
            continue;
        }
        let Ok(handle) = world.handle_of(PersistentId(*entity)) else {
            continue;
        };
        let walker = world.has_row(handle, "Character");
        let dynamic = matches!(
            world.get(handle, "RigidBody", "kind"),
            Ok(Value::Text(kind)) if kind == "dynamic"
        );
        let size = solid.size();
        let shape = if walker {
            ColliderShape::Capsule {
                radius: (size.x.max(size.z) * 0.5).max(0.05),
                height: size.y.max(0.2),
            }
        } else if solid.is_box {
            ColliderShape::Box { size }
        } else if dynamic {
            let mut points = solid.points.clone();
            if points.len() > MAX_HULL_POINTS {
                let stride = points.len().div_ceil(MAX_HULL_POINTS);
                points = points.into_iter().step_by(stride).collect();
            }
            ColliderShape::Convex { points }
        } else {
            ColliderShape::TriMesh {
                points: solid.points.clone(),
                triangles: solid.triangles.clone(),
            }
        };
        let collider = ColliderDescription {
            collider: AUTO_COLLIDER_FLAG | *entity,
            owner: *entity,
            name: "sólido".to_string(),
            shape,
            center: if solid.is_box || walker {
                solid.center()
            } else {
                Vec3::ZERO
            },
            rotation_degrees: Vec3::ZERO,
            trigger: false,
            friction: 0.5,
            restitution: 0.0,
            layer: DEFAULT_LAYER,
            mask: DEFAULT_MASK,
        };
        match check_collider(&collider) {
            Ok(()) => automatic.push(collider),
            Err(error) => scene
                .problems
                .push(format!("objeto #{entity}: sem colisão automática: {error}")),
        }
    }
    for collider in automatic {
        owners.insert(collider.owner, ());
        scene.colliders.push(collider);
    }

    // Characters walk: their body is kinematic, with or without a `RigidBody` row.
    for handle in world.entities_with_table("Character").unwrap_or_default() {
        let Ok(PersistentId(id)) = world.persistent_id(handle) else {
            continue;
        };
        let description = match read_character(world, handle, id) {
            Ok(description) => description,
            Err(problem) => {
                scene.problems.push(format!("personagem #{id}: {problem}"));
                continue;
            }
        };
        if let Err(error) = crate::character::check_character(&description) {
            scene.problems.push(format!("personagem #{id}: {error}"));
            continue;
        }
        if !scene
            .colliders
            .iter()
            .any(|collider| collider.owner == id && !collider.trigger)
        {
            scene.problems.push(format!(
                "personagem #{id}: precisa de uma colisão sólida (uma cápsula é o melhor); ficou parado"
            ));
            continue;
        }
        match scene.bodies.iter_mut().find(|body| body.entity == id) {
            Some(body) if body.kind == BodyKind::Static => {
                body.kind = BodyKind::Kinematic;
            }
            Some(body) if body.kind == BodyKind::Dynamic => {
                scene.problems.push(format!(
                    "personagem #{id}: o corpo é dinâmico; o personagem precisa de um corpo cinemático"
                ));
                continue;
            }
            Some(_) => {}
            None => {
                let mut body = BodyDescription::fixed(id, transform_of(world, handle));
                body.kind = BodyKind::Kinematic;
                scene.bodies.push(body);
                with_body.insert(id, ());
            }
        }
        scene.characters.push(description);
    }

    // Objects with colliders and no body stand still.
    for owner in owners.keys() {
        if with_body.contains_key(owner) {
            continue;
        }
        match world.handle_of(PersistentId(*owner)) {
            Ok(handle) => scene
                .bodies
                .push(BodyDescription::fixed(*owner, transform_of(world, handle))),
            Err(_) => {
                scene.problems.push(format!(
                    "o objeto #{owner} tem colisões mas não existe mais"
                ));
            }
        }
    }
    // Colliders whose owner is gone are left out (the problem above names it).
    let known: BTreeMap<u64, ()> = scene.bodies.iter().map(|body| (body.entity, ())).collect();
    scene
        .colliders
        .retain(|collider| known.contains_key(&collider.owner));
    scene
}

fn transform_of(world: &World, handle: EntityHandle) -> Transform {
    let vector = |column: &str, fallback: Vec3| match world.get(handle, "Transform", column) {
        Ok(Value::Vec3(value)) => *value,
        _ => fallback,
    };
    Transform {
        position: vector("position", Vec3::ZERO),
        rotation_degrees: vector("rotation_degrees", Vec3::ZERO),
        scale: vector("scale", Vec3::ONE),
    }
}

fn float(world: &World, handle: EntityHandle, table: &str, column: &str) -> Result<f32, String> {
    match world.get(handle, table, column) {
        Ok(Value::Float(value)) => Ok(*value as f32),
        Ok(Value::Int(value)) => Ok(*value as f32),
        _ => Err(format!("a coluna `{column}` não é um número")),
    }
}

fn read_collider(
    world: &World,
    handle: EntityHandle,
    id: u64,
    solids: &BTreeMap<u64, SolidShape>,
) -> Result<ColliderDescription, String> {
    let owner = match world.get(handle, "Collider", "owner") {
        Ok(Value::Int(owner)) if *owner > 0 => *owner as u64,
        _ => return Err("não diz a que objeto pertence (`owner`)".to_string()),
    };
    if owner == id {
        return Err("não pode pertencer a si mesma".to_string());
    }
    let text = |column: &str| match world.get(handle, "Collider", column) {
        Ok(Value::Text(value)) => Ok(value.clone()),
        _ => Err(format!("a coluna `{column}` não é texto")),
    };
    let vector = |column: &str| match world.get(handle, "Collider", column) {
        Ok(Value::Vec3(value)) => Ok(*value),
        _ => Err(format!("a coluna `{column}` não é um Vec3")),
    };
    let shape = match text("shape")?.as_str() {
        "box" => ColliderShape::Box {
            size: vector("size")?,
        },
        "sphere" => ColliderShape::Sphere {
            radius: float(world, handle, "Collider", "radius")?,
        },
        "capsule" => ColliderShape::Capsule {
            radius: float(world, handle, "Collider", "radius")?,
            height: float(world, handle, "Collider", "height")?,
        },
        "convex" => {
            let from_mesh = world
                .handle_of(PersistentId(owner))
                .ok()
                .and_then(
                    |owner_handle| match world.get(owner_handle, "Mesh", "geometry") {
                        Ok(Value::Mesh(mesh)) => Some(mesh.positions.clone()),
                        _ => None,
                    },
                );
            let mut points = from_mesh
                .or_else(|| solids.get(&owner).map(|solid| solid.points.clone()))
                .ok_or_else(|| "forma convexa num objeto sem malha".to_string())?;
            if points.len() > MAX_HULL_POINTS {
                // Evenly spaced points keep the outline of a dense mesh.
                let stride = points.len().div_ceil(MAX_HULL_POINTS);
                points = points.into_iter().step_by(stride).collect();
            }
            ColliderShape::Convex { points }
        }
        other => {
            return Err(format!(
                "forma `{other}` desconhecida (box, sphere, capsule ou convex)"
            ));
        }
    };
    let trigger = matches!(
        world.get(handle, "Collider", "trigger"),
        Ok(Value::Bool(true))
    );
    Ok(ColliderDescription {
        collider: id,
        owner,
        name: text("name")?,
        shape,
        center: vector("center")?,
        rotation_degrees: vector("rotation_degrees")?,
        trigger,
        friction: float(world, handle, "Collider", "friction")?,
        restitution: float(world, handle, "Collider", "restitution")?,
        layer: bits(world, handle, "layer", DEFAULT_LAYER)?,
        mask: bits(world, handle, "mask", DEFAULT_MASK)?,
    })
}

/// A layer or mask column: a whole number from 0 to 4294967295 (older projects have no column).
fn bits(world: &World, handle: EntityHandle, column: &str, default: u32) -> Result<u32, String> {
    match world.get(handle, "Collider", column) {
        Ok(Value::Int(value)) => {
            u32::try_from(*value).map_err(|_| format!("`{column}` vai de 0 a {}", u32::MAX))
        }
        Ok(_) => Err(format!("a coluna `{column}` não é um número inteiro")),
        Err(_) => Ok(default),
    }
}

fn read_character(
    world: &World,
    handle: EntityHandle,
    id: u64,
) -> Result<CharacterDescription, String> {
    let mut description = CharacterDescription::new(id);
    description.max_slope_degrees = float(world, handle, "Character", "max_slope_degrees")?;
    description.step_height = float(world, handle, "Character", "step_height")?;
    description.jump_speed = float(world, handle, "Character", "jump_speed")?;
    description.coyote_seconds = float(world, handle, "Character", "coyote_seconds")?;
    description.gravity_scale = float(world, handle, "Character", "gravity_scale")?;
    Ok(description)
}

fn read_body(world: &World, handle: EntityHandle, id: u64) -> Result<BodyDescription, String> {
    let kind = match world.get(handle, "RigidBody", "kind") {
        Ok(Value::Text(kind)) => BodyKind::parse(kind)
            .ok_or_else(|| format!("tipo `{kind}` desconhecido (static, dynamic ou kinematic)"))?,
        _ => return Err("a coluna `kind` não é texto".to_string()),
    };
    Ok(BodyDescription {
        entity: id,
        kind,
        transform: transform_of(world, handle),
        mass: float(world, handle, "RigidBody", "mass")?,
        gravity_scale: float(world, handle, "RigidBody", "gravity_scale")?,
        linear_damping: float(world, handle, "RigidBody", "linear_damping")?,
        angular_damping: float(world, handle, "RigidBody", "angular_damping")?,
        lock_rotation: matches!(
            world.get(handle, "RigidBody", "lock_rotation"),
            Ok(Value::Bool(true))
        ),
    })
}

#[cfg(test)]
mod tests {
    use petunia_world::{PersistentId, World};
    use serde_json::json;

    use super::*;

    fn spawn(world: &mut World, rows: serde_json::Value) -> u64 {
        let handle = world.spawn().expect("room for one more");
        for (table, values) in rows.as_object().expect("rows") {
            world
                .insert_row_json(handle, table, values.as_object().expect("values"))
                .expect("valid row");
        }
        world.persistent_id(handle).expect("alive").0
    }

    fn standard_world() -> World {
        let mut world = World::new(7);
        petunia_project::register_standard_tables(&mut world).expect("tables");
        world
    }

    #[test]
    fn a_world_with_named_colliders_becomes_a_simulation() {
        let mut world = standard_world();
        let floor = spawn(
            &mut world,
            json!({ "Transform": { "position": { "x": 0.0, "y": -0.5, "z": 0.0 } } }),
        );
        spawn(
            &mut world,
            json!({ "Collider": { "owner": floor, "name": "chão", "size": { "x": 20.0, "y": 1.0, "z": 20.0 } } }),
        );
        let ball = spawn(
            &mut world,
            json!({
                "Transform": { "position": { "x": 0.0, "y": 3.0, "z": 0.0 } },
                "RigidBody": { "kind": "dynamic", "mass": 2.0 }
            }),
        );
        spawn(
            &mut world,
            json!({ "Collider": { "owner": ball, "name": "corpo", "shape": "sphere", "radius": 0.5 } }),
        );
        spawn(
            &mut world,
            json!({ "Collider": { "owner": ball, "name": "pés", "shape": "sphere", "radius": 0.2, "center": { "x": 0.0, "y": -0.4, "z": 0.0 }, "trigger": true } }),
        );
        // Broken ones are reported and left out.
        spawn(
            &mut world,
            json!({ "Collider": { "owner": ball, "name": "estranha", "shape": "torus" } }),
        );
        spawn(
            &mut world,
            json!({ "Collider": { "owner": 999, "name": "órfã" } }),
        );
        spawn(
            &mut world,
            json!({ "Collider": { "owner": ball, "name": "negativa", "shape": "sphere", "radius": -1.0 } }),
        );

        let scene = describe(&world, &BTreeMap::new());
        assert_eq!(scene.problems.len(), 3, "{:?}", scene.problems);
        assert!(
            scene
                .problems
                .iter()
                .any(|problem| problem.contains("torus"))
        );
        assert!(
            scene
                .problems
                .iter()
                .any(|problem| problem.contains("#999"))
        );
        let mut names: Vec<&str> = scene
            .colliders
            .iter()
            .map(|collider| collider.name.as_str())
            .collect();
        names.sort();
        assert_eq!(names, ["chão", "corpo", "pés"]);
        assert_eq!(
            scene.bodies.len(),
            2,
            "the ball's body and a fixed one for the floor"
        );

        let mut simulation = scene
            .build(PhysicsSettings::default())
            .expect("valid scene");
        let mut feet_on_floor = false;
        for _ in 0..240 {
            for event in simulation.step() {
                feet_on_floor |= event.first.name == "chão" && event.second.name == "pés";
            }
        }
        assert!(feet_on_floor, "the feet trigger reports touching the floor");
        let pose = simulation
            .poses()
            .into_iter()
            .find(|pose| pose.entity == ball)
            .expect("the ball moves");
        assert!((pose.position.y - 0.5).abs() < 0.05, "{:?}", pose.position);
        assert!(world.handle_of(PersistentId(ball)).is_ok());
    }

    #[test]
    fn convex_colliders_take_the_points_from_the_caller() {
        let mut world = standard_world();
        let rock = spawn(&mut world, json!({ "Transform": {} }));
        spawn(
            &mut world,
            json!({ "Collider": { "owner": rock, "name": "rocha", "shape": "convex" } }),
        );
        let without = describe(&world, &BTreeMap::new());
        assert!(
            without.colliders.is_empty() && without.problems.len() == 1,
            "{:?}",
            without.problems
        );
        let corners: Vec<[f32; 3]> = [-1.0f32, 1.0]
            .iter()
            .flat_map(|&x| [-1.0f32, 1.0].map(move |y| [x, y, 0.0]))
            .flat_map(|point| [-1.0f32, 1.0].map(move |z| [point[0], point[1], z]))
            .collect();
        let mut solids = BTreeMap::new();
        solids.insert(
            rock,
            SolidShape::from_triangles(&corners, &[0, 1, 2], false).expect("valid points"),
        );
        let with = describe(&world, &solids);
        assert_eq!(with.colliders.len(), 1, "{:?}", with.problems);
        assert!(with.build(PhysicsSettings::default()).is_ok());
    }
}

//! Parent links and fixed cameras, as columns, not as a scene-graph class.
//!
//! A `Transform.parent` of 0 means the object sits in the world. Any other value is the persistent
//! id of the parent. The stored position stays local, so moving the parent moves the children.
//! A `Shot` row is one fixed camera: its transform is the eye, and `half_extents` is the box of
//! the room that turns it on. The highest priority shot that contains a point wins.

use petunia_math::{Transform, Vec3};

use crate::ids::{EntityHandle, PersistentId};
use crate::value::Value;
use crate::world::World;

const MAX_PARENT_DEPTH: usize = 32;

/// The persistent id in `Transform.parent`, or 0 when the object is a root.
pub fn parent_id(world: &World, handle: EntityHandle) -> u64 {
    match world.get(handle, "Transform", "parent") {
        Ok(Value::Int(id)) if *id > 0 => *id as u64,
        _ => 0,
    }
}

fn local_transform(world: &World, handle: EntityHandle) -> Transform {
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

/// The transform in world space, walking parents. A cycle or a missing parent stops the walk
/// and keeps what was already composed, so a broken link cannot hang the frame.
pub fn composed_transform(world: &World, handle: EntityHandle) -> Transform {
    let mut locals = Vec::new();
    let mut seen = Vec::new();
    let mut current = handle;
    for _ in 0..MAX_PARENT_DEPTH {
        let Ok(id) = world.persistent_id(current) else {
            break;
        };
        if seen.contains(&id.0) {
            break;
        }
        seen.push(id.0);
        locals.push(local_transform(world, current));
        let parent = parent_id(world, current);
        if parent == 0 {
            break;
        }
        let Ok(parent_handle) = world.handle_of(PersistentId(parent)) else {
            break;
        };
        current = parent_handle;
    }
    let mut composed = Transform::IDENTITY;
    for local in locals.iter().rev() {
        composed = composed.compose(*local);
    }
    composed
}

/// One fixed camera that can own the view while the player stands inside its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotPose {
    pub entity: u64,
    pub position: Vec3,
    pub yaw_degrees: f32,
    pub pitch_degrees: f32,
    pub fov_degrees: f32,
    pub priority: i64,
}

/// The fixed camera on one object, ignoring whether the player is inside its box.
pub fn shot_of(world: &World, entity: u64) -> Option<ShotPose> {
    let handle = world.handle_of(PersistentId(entity)).ok()?;
    if world.get(handle, "Shot", "enabled").is_err() {
        return None;
    }
    if matches!(world.get(handle, "Shot", "enabled"), Ok(Value::Bool(false))) {
        return None;
    }
    let pose = composed_transform(world, handle);
    let fov = match world.get(handle, "Shot", "fov_degrees") {
        Ok(Value::Float(value)) if value.is_finite() && *value > 1.0 => *value as f32,
        _ => 50.0,
    };
    let priority = match world.get(handle, "Shot", "priority") {
        Ok(Value::Int(value)) => *value,
        _ => 0,
    };
    Some(ShotPose {
        entity,
        position: pose.position,
        yaw_degrees: pose.rotation_degrees.y,
        pitch_degrees: pose.rotation_degrees.x,
        fov_degrees: fov,
        priority,
    })
}

/// The shot whose box contains `point`, preferring a higher priority and then a lower id.
pub fn active_shot(world: &World, point: Vec3) -> Option<ShotPose> {
    let mut chosen: Option<ShotPose> = None;
    for handle in world.entities_with_table("Shot").unwrap_or_default() {
        if matches!(world.get(handle, "Shot", "enabled"), Ok(Value::Bool(false))) {
            continue;
        }
        let pose = composed_transform(world, handle);
        let half = match world.get(handle, "Shot", "half_extents") {
            Ok(Value::Vec3(extents)) => *extents,
            _ => Vec3::new(6.0, 4.0, 6.0),
        };
        if !inside_box(point, pose.position, half) {
            continue;
        }
        let priority = match world.get(handle, "Shot", "priority") {
            Ok(Value::Int(value)) => *value,
            _ => 0,
        };
        let fov = match world.get(handle, "Shot", "fov_degrees") {
            Ok(Value::Float(value)) if value.is_finite() && *value > 1.0 => *value as f32,
            _ => 50.0,
        };
        let entity = world.persistent_id(handle).map_or(0, |id| id.0);
        let candidate = ShotPose {
            entity,
            position: pose.position,
            yaw_degrees: pose.rotation_degrees.y,
            pitch_degrees: pose.rotation_degrees.x,
            fov_degrees: fov,
            priority,
        };
        let replace = match chosen {
            None => true,
            Some(current) => {
                candidate.priority > current.priority
                    || (candidate.priority == current.priority && candidate.entity < current.entity)
            }
        };
        if replace {
            chosen = Some(candidate);
        }
    }
    chosen
}

fn inside_box(point: Vec3, centre: Vec3, half: Vec3) -> bool {
    if !half.is_finite() || half.x <= 0.0 || half.y <= 0.0 || half.z <= 0.0 {
        return false;
    }
    let delta = point - centre;
    delta.x.abs() <= half.x && delta.y.abs() <= half.y && delta.z.abs() <= half.z
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::TableSpec;
    use crate::value::ValueKind;
    use crate::world::World;

    fn scene() -> World {
        let mut world = World::new(1);
        world
            .define_table(
                TableSpec::new("Transform")
                    .column("position", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("rotation_degrees", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("scale", ValueKind::Vec3, Value::Vec3(Vec3::ONE))
                    .column("pivot", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("parent", ValueKind::Int, Value::Int(0)),
            )
            .expect("transform");
        world
            .define_table(
                TableSpec::new("Shot")
                    .column("fov_degrees", ValueKind::Float, Value::Float(50.0))
                    .column("priority", ValueKind::Int, Value::Int(0))
                    .column(
                        "half_extents",
                        ValueKind::Vec3,
                        Value::Vec3(Vec3::new(4.0, 3.0, 4.0)),
                    )
                    .column("enabled", ValueKind::Bool, Value::Bool(true)),
            )
            .expect("shot");
        world
    }

    fn place(world: &mut World, position: Vec3, parent: i64) -> u64 {
        let handle = world.spawn().expect("spawn");
        world
            .insert_row_json(
                handle,
                "Transform",
                &serde_json::json!({
                    "position": { "x": position.x, "y": position.y, "z": position.z },
                    "parent": parent,
                })
                .as_object()
                .expect("object")
                .clone(),
            )
            .expect("row");
        world.persistent_id(handle).expect("id").0
    }

    #[test]
    fn a_child_follows_the_parent_yaw() {
        let mut world = scene();
        let parent = place(&mut world, Vec3::new(0.0, 0.0, 0.0), 0);
        let parent_handle = world.handle_of(PersistentId(parent)).expect("parent");
        world
            .set(
                parent_handle,
                "Transform",
                "rotation_degrees",
                Value::Vec3(Vec3::new(0.0, 90.0, 0.0)),
            )
            .expect("yaw");
        let child = place(&mut world, Vec3::new(2.0, 1.0, 0.0), parent as i64);
        let child_handle = world.handle_of(PersistentId(child)).expect("child");
        let composed = composed_transform(&world, child_handle);
        assert!((composed.position.z + 2.0).abs() < 0.001, "{composed:?}");
        assert!((composed.position.y - 1.0).abs() < 0.001);
    }

    #[test]
    fn a_cycle_stops_instead_of_looping() {
        let mut world = scene();
        let first = place(&mut world, Vec3::new(1.0, 0.0, 0.0), 0);
        let second = place(&mut world, Vec3::new(1.0, 0.0, 0.0), first as i64);
        let first_handle = world.handle_of(PersistentId(first)).expect("first");
        world
            .set(
                first_handle,
                "Transform",
                "parent",
                Value::Int(second as i64),
            )
            .expect("cycle");
        let composed = composed_transform(&world, first_handle);
        assert!(composed.position.is_finite());
    }

    #[test]
    fn the_higher_priority_shot_that_contains_the_player_wins() {
        let mut world = scene();
        let wide = place(&mut world, Vec3::ZERO, 0);
        let wide_handle = world.handle_of(PersistentId(wide)).expect("wide");
        world
            .insert_row_json(
                wide_handle,
                "Shot",
                &serde_json::json!({ "priority": 1, "half_extents": { "x": 10.0, "y": 4.0, "z": 10.0 } })
                    .as_object()
                    .expect("object")
                    .clone(),
            )
            .expect("wide shot");
        let tight = place(&mut world, Vec3::new(2.0, 1.5, 0.0), 0);
        let tight_handle = world.handle_of(PersistentId(tight)).expect("tight");
        world
            .set(
                tight_handle,
                "Transform",
                "rotation_degrees",
                Value::Vec3(Vec3::new(-10.0, 180.0, 0.0)),
            )
            .expect("look");
        world
            .insert_row_json(
                tight_handle,
                "Shot",
                &serde_json::json!({ "priority": 5, "fov_degrees": 40.0, "half_extents": { "x": 3.0, "y": 3.0, "z": 3.0 } })
                    .as_object()
                    .expect("object")
                    .clone(),
            )
            .expect("tight shot");
        let inside = active_shot(&world, Vec3::new(2.0, 1.0, 0.0)).expect("shot");
        assert_eq!(inside.entity, tight);
        assert!((inside.yaw_degrees - 180.0).abs() < 0.001);
        assert!((inside.fov_degrees - 40.0).abs() < 0.001);
        let outside = active_shot(&world, Vec3::new(2.0, 1.0, 0.0) + Vec3::new(0.0, 0.0, 8.0));
        assert_eq!(outside.expect("wide still covers").entity, wide);
        assert!(active_shot(&world, Vec3::new(40.0, 0.0, 0.0)).is_none());
    }
}

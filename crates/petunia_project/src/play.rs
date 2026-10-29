//! What a scene needs to be played that the person did not set up.

use petunia_math::Vec3;
use petunia_world::{Value, World, WorldError};
use serde_json::{Map, Value as Json, json};

/// Eye height the `PlayerStart` marks: the character's feet are this far below it.
pub const EYE_HEIGHT: f32 = 1.7;
/// Size of the stand-in hero (a cylinder brush; its collider is a capsule made from it).
const HERO_SIZE: [f32; 3] = [0.6, 1.8, 0.6];

/// Where the player starts: the eye position and the facing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerStart {
    pub eye: Vec3,
    pub yaw_degrees: f32,
}

/// The `PlayerStart` that should be used. A start on a drawn object (a floor with the table added
/// by accident, for example) loses to a dedicated marker; among markers, the highest wins, then
/// the lowest id. The scene keeps working even when someone adds the table to the wrong object.
pub fn player_start(world: &World) -> Option<PlayerStart> {
    let mut starts = world.entities_with_table("PlayerStart").ok()?;
    starts.retain(|handle| world.get(*handle, "Transform", "position").is_ok());
    let drawn = |handle| {
        world.has_row(handle, "Brush")
            || world.has_row(handle, "Mesh")
            || world.has_row(handle, "Sculpt")
    };
    let height = |handle| match world.get(handle, "Transform", "position") {
        Ok(Value::Vec3(position)) => position.y,
        _ => 0.0,
    };
    let id = |handle| world.persistent_id(handle).map_or(u64::MAX, |id| id.0);
    starts.sort_by(|left, right| {
        drawn(*left)
            .cmp(&drawn(*right))
            .then_with(|| {
                height(*right)
                    .partial_cmp(&height(*left))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| id(*left).cmp(&id(*right)))
    });
    let start = *starts.first()?;
    let eye = match world.get(start, "Transform", "position") {
        Ok(Value::Vec3(position)) => *position,
        _ => Vec3::ZERO,
    };
    let yaw_degrees = match world.get(start, "PlayerStart", "yaw_degrees") {
        Ok(Value::Float(yaw)) => *yaw as f32,
        _ => 0.0,
    };
    Some(PlayerStart { eye, yaw_degrees })
}

fn object(value: Json) -> Map<String, Json> {
    match value {
        Json::Object(map) => map,
        _ => Map::new(),
    }
}

/// True when some character is driven by the keyboard (`Character.player`).
pub fn has_player(world: &World) -> bool {
    world
        .entities_with_table("Character")
        .unwrap_or_default()
        .into_iter()
        .any(|handle| {
            matches!(
                world.get(handle, "Character", "player"),
                Ok(Value::Bool(true))
            )
        })
}

/// Makes sure there is someone to play: if no character is driven by the keyboard, a stand-in
/// hero is added at the `PlayerStart` (nothing is added when the scene has none). Meant for the
/// **copy** of the world a game runs on; it returns the hero's persistent id.
pub fn ensure_player(world: &mut World) -> Result<Option<u64>, WorldError> {
    if has_player(world) {
        return Ok(None);
    }
    let Some(start) = player_start(world) else {
        return Ok(None);
    };
    let eye = start.eye;
    let yaw = start.yaw_degrees;
    let hero = world.spawn()?;
    let center = Vec3::new(eye.x, eye.y - EYE_HEIGHT + HERO_SIZE[1] * 0.5 + 0.05, eye.z);
    world.insert_row_json(hero, "Name", &object(json!({ "text": "Jogador" })))?;
    world.insert_row_json(
        hero,
        "Transform",
        &object(json!({
            "position": { "x": center.x, "y": center.y, "z": center.z },
            "rotation_degrees": { "x": 0.0, "y": -yaw, "z": 0.0 }
        })),
    )?;
    world.insert_row_json(
        hero,
        "Brush",
        &object(json!({
            "shape": "cylinder",
            "size": { "x": HERO_SIZE[0], "y": HERO_SIZE[1], "z": HERO_SIZE[2] },
            "color": { "r": 0.95, "g": 0.55, "b": 0.75, "a": 1.0 }
        })),
    )?;
    world.insert_row_json(hero, "Character", &object(json!({ "player": true })))?;
    Ok(Some(world.persistent_id(hero)?.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::register_standard_tables;

    fn world() -> World {
        let mut world = World::new(1);
        register_standard_tables(&mut world).expect("tables");
        world
    }

    #[test]
    fn a_start_on_a_drawn_floor_does_not_win_over_the_marker() {
        let mut scene = world();
        let floor = scene.spawn().expect("floor");
        scene
            .insert_row_json(
                floor,
                "Transform",
                &object(json!({ "position": { "x": 0.0, "y": -0.25, "z": 0.0 } })),
            )
            .expect("transform");
        scene
            .insert_row_json(
                floor,
                "Brush",
                &object(json!({ "shape": "box", "size": { "x": 24.0, "y": 0.5, "z": 24.0 } })),
            )
            .expect("brush");
        scene
            .insert_row_json(floor, "PlayerStart", &object(json!({ "yaw_degrees": 0.0 })))
            .expect("start on the floor");

        let marker = scene.spawn().expect("marker");
        scene
            .insert_row_json(
                marker,
                "Transform",
                &object(json!({ "position": { "x": 0.0, "y": 1.7, "z": 9.0 } })),
            )
            .expect("transform");
        scene
            .insert_row_json(
                marker,
                "PlayerStart",
                &object(json!({ "yaw_degrees": 90.0 })),
            )
            .expect("marker start");

        let chosen = player_start(&scene).expect("a start");
        assert_eq!(chosen.eye.y, 1.7);
        assert_eq!(chosen.yaw_degrees, 90.0);
        let hero = ensure_player(&mut scene).expect("ok").expect("hero");
        let handle = scene
            .handle_of(petunia_world::PersistentId(hero))
            .expect("alive");
        let Ok(Value::Vec3(position)) = scene.get(handle, "Transform", "position") else {
            panic!("no position");
        };
        assert!(
            (position.y - 0.95).abs() < 1e-4,
            "the hero stands on the floor under the marker: {}",
            position.y
        );
    }

    #[test]
    fn a_hero_appears_at_the_start_only_when_there_is_none() {
        let mut empty = world();
        assert_eq!(
            ensure_player(&mut empty).expect("ok"),
            None,
            "no start, no hero"
        );

        let mut scene = world();
        let start = scene.spawn().expect("room");
        scene
            .insert_row_json(
                start,
                "Transform",
                &object(json!({ "position": { "x": 2.0, "y": 1.7, "z": 9.0 } })),
            )
            .expect("row");
        scene
            .insert_row_json(
                start,
                "PlayerStart",
                &object(json!({ "yaw_degrees": 90.0 })),
            )
            .expect("row");
        let hero = ensure_player(&mut scene)
            .expect("ok")
            .expect("a hero was made");
        assert!(has_player(&scene));
        let handle = scene
            .handle_of(petunia_world::PersistentId(hero))
            .expect("alive");
        let Ok(Value::Vec3(position)) = scene.get(handle, "Transform", "position") else {
            panic!("no position");
        };
        assert!((position.x - 2.0).abs() < 1e-5 && (position.z - 9.0).abs() < 1e-5);
        assert!(
            (position.y - 0.95).abs() < 1e-4,
            "feet on the floor under the eyes: {}",
            position.y
        );
        assert_eq!(
            ensure_player(&mut scene).expect("ok"),
            None,
            "never two heroes"
        );
    }
}

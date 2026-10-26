//! What a scene needs to be played that the person did not set up.

use petunia_math::Vec3;
use petunia_world::{Value, World, WorldError};
use serde_json::{Map, Value as Json, json};

/// Eye height the `PlayerStart` marks: the character's feet are this far below it.
pub const EYE_HEIGHT: f32 = 1.7;
/// Size of the stand-in hero (a cylinder brush; its collider is a capsule made from it).
const HERO_SIZE: [f32; 3] = [0.6, 1.8, 0.6];

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
    let Some(start) = world
        .entities_with_table("PlayerStart")
        .unwrap_or_default()
        .first()
        .copied()
    else {
        return Ok(None);
    };
    let eye = match world.get(start, "Transform", "position") {
        Ok(Value::Vec3(position)) => *position,
        _ => Vec3::ZERO,
    };
    let yaw = match world.get(start, "PlayerStart", "yaw_degrees") {
        Ok(Value::Float(yaw)) => *yaw,
        _ => 0.0,
    };
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

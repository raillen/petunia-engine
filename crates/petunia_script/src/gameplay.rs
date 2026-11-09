//! Interaction and health (DR-44). Both are tables on an object. The host notices a key press
//! and a life total that crosses zero; scripts hear `on_event` with `"interact"` or `"died"`.

use std::collections::BTreeMap;

use petunia_math::Vec3;
use petunia_world::{PersistentId, Value, World};

/// What the player can use right now: the nearest enabled `Interact` inside its radius.
#[derive(Clone, Debug, PartialEq)]
pub struct InteractOffer {
    pub entity: u64,
    pub prompt: String,
    pub distance: f32,
}

/// The closest `Interact` to the keyboard-driven character, or nothing in range.
pub fn nearest_interact(world: &World) -> Option<InteractOffer> {
    let player = player_feet(world)?;
    let mut best: Option<InteractOffer> = None;
    let handles = world.entities_with_table("Interact").unwrap_or_default();
    for handle in handles {
        if matches!(
            world.get(handle, "Interact", "enabled"),
            Ok(Value::Bool(false))
        ) {
            continue;
        }
        let Ok(PersistentId(entity)) = world.persistent_id(handle) else {
            continue;
        };
        if entity == player.entity {
            continue;
        }
        let Some(radius) = positive_float(world, handle, "Interact", "radius") else {
            continue;
        };
        let place = petunia_world::composed_transform(world, handle).position;
        let distance = (place - player.feet).length();
        if distance > radius {
            continue;
        }
        let prompt = match world.get(handle, "Interact", "prompt") {
            Ok(Value::Text(text)) if !text.is_empty() => text.clone(),
            _ => "Interagir".to_string(),
        };
        let closer = match &best {
            None => true,
            Some(offer) => {
                let delta = distance - offer.distance;
                delta < -1.0e-4 || (delta.abs() <= 1.0e-4 && entity < offer.entity)
            }
        };
        if closer {
            best = Some(InteractOffer {
                entity,
                prompt,
                distance,
            });
        }
    }
    best
}

/// `Health.current` of every object that has the table. Missing or non-finite totals are left out.
pub fn health_points(world: &World) -> BTreeMap<u64, f64> {
    let mut points = BTreeMap::new();
    let handles = world.entities_with_table("Health").unwrap_or_default();
    for handle in handles {
        let Ok(PersistentId(entity)) = world.persistent_id(handle) else {
            continue;
        };
        let Ok(Value::Float(current)) = world.get(handle, "Health", "current") else {
            continue;
        };
        if current.is_finite() {
            points.insert(entity, *current);
        }
    }
    points
}

/// Objects whose life was above zero and is now zero or below.
pub fn health_deaths(before: &BTreeMap<u64, f64>, after: &BTreeMap<u64, f64>) -> Vec<u64> {
    after
        .iter()
        .filter(|(entity, now)| before.get(entity).is_some_and(|was| *was > 0.0) && **now <= 0.0)
        .map(|(entity, _)| *entity)
        .collect()
}

struct PlayerFeet {
    entity: u64,
    feet: Vec3,
}

fn player_feet(world: &World) -> Option<PlayerFeet> {
    let handles = world.entities_with_table("Character").ok()?;
    let handle = handles.into_iter().find(|handle| {
        matches!(
            world.get(*handle, "Character", "player"),
            Ok(Value::Bool(true))
        )
    })?;
    let entity = world.persistent_id(handle).ok()?.0;
    let feet = petunia_world::composed_transform(world, handle).position;
    Some(PlayerFeet { entity, feet })
}

fn positive_float(
    world: &World,
    handle: petunia_world::EntityHandle,
    table: &str,
    column: &str,
) -> Option<f32> {
    match world.get(handle, table, column) {
        Ok(Value::Float(value)) if value.is_finite() && *value > 0.0 => Some(*value as f32),
        _ => None,
    }
}

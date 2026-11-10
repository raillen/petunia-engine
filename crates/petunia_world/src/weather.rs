//! Weather table and specifications (DR-53 Frente 6).
//!
//! A scene can have one or more `Weather` entities. Each defines a preset, an hour, a weight and
//! an optional volume (`""` for global, or `"box"`). When inside a box volume, that volume wins
//! over global weather. Multiple matching weathers blend by weight.

use petunia_math::Vec3;
use serde::{Deserialize, Serialize};

use crate::world::World;
use crate::{EntityHandle, Value};

/// Table name for weather components.
pub const WEATHER_TABLE: &str = "Weather";

/// A weather definition from an entity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeatherSpec {
    pub preset: String,
    pub hours: f32,
    pub weight: f32,
    pub volume: String,
}

impl Default for WeatherSpec {
    fn default() -> Self {
        Self {
            preset: "clear".to_string(),
            hours: 13.0,
            weight: 1.0,
            volume: String::new(),
        }
    }
}

/// Reads the weather specification of an entity, if it has a `Weather` row.
#[must_use]
pub fn weather_of(world: &World, handle: EntityHandle) -> Option<WeatherSpec> {
    if !world.has_row(handle, WEATHER_TABLE) {
        return None;
    }
    let preset = match world.get(handle, WEATHER_TABLE, "preset") {
        Ok(Value::Text(text)) => text.clone(),
        _ => "clear".to_string(),
    };
    let hours = match world.get(handle, WEATHER_TABLE, "hours") {
        Ok(Value::Float(value)) if value.is_finite() => (*value as f32).rem_euclid(24.0),
        _ => 13.0,
    };
    let weight = match world.get(handle, WEATHER_TABLE, "weight") {
        Ok(Value::Float(value)) if value.is_finite() => (*value as f32).clamp(0.0, 1.0),
        _ => 1.0,
    };
    let volume = match world.get(handle, WEATHER_TABLE, "volume") {
        Ok(Value::Text(text)) => text.clone(),
        _ => String::new(),
    };
    Some(WeatherSpec {
        preset,
        hours,
        weight,
        volume,
    })
}

/// A weather volume with its spatial bounds and specification.
#[derive(Clone, Debug, PartialEq)]
pub struct EvaluatedWeather {
    pub spec: WeatherSpec,
    pub is_volume: bool,
    pub center: Vec3,
    pub half_size: Vec3,
}

impl EvaluatedWeather {
    /// Tests whether a world-space point is inside this weather's volume.
    #[must_use]
    pub fn contains_point(&self, point: Vec3) -> bool {
        if !self.is_volume {
            return true;
        }
        (point.x - self.center.x).abs() <= self.half_size.x
            && (point.y - self.center.y).abs() <= self.half_size.y
            && (point.z - self.center.z).abs() <= self.half_size.z
    }
}

/// Extracts all weather entities with their world-space bounding volumes.
#[must_use]
pub fn extract_all_weathers(world: &World) -> Vec<EvaluatedWeather> {
    let Ok(handles) = world.entities_with_table(WEATHER_TABLE) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    for handle in handles {
        let Some(spec) = weather_of(world, handle) else {
            continue;
        };
        let is_volume = !spec.volume.is_empty();
        let center = match world.get(handle, "Transform", "position") {
            Ok(Value::Vec3(position)) => *position,
            _ => Vec3::ZERO,
        };
        let scale = match world.get(handle, "Transform", "scale") {
            Ok(Value::Vec3(scale_vec)) => *scale_vec,
            _ => Vec3::ONE,
        };
        let brush_size = match world.get(handle, "Brush", "size") {
            Ok(Value::Vec3(size)) => *size,
            _ => Vec3::ONE,
        };
        let half_size = Vec3::new(
            (scale.x * brush_size.x * 0.5).abs().max(0.1),
            (scale.y * brush_size.y * 0.5).abs().max(0.1),
            (scale.z * brush_size.z * 0.5).abs().max(0.1),
        );
        result.push(EvaluatedWeather {
            spec,
            is_volume,
            center,
            half_size,
        });
    }
    result
}

/// Returns the active weathers at the given observer position.
///
/// If any volume weathers enclose the observer, those volume weathers win over the global ones.
/// Otherwise, global weathers (with empty `volume`) apply.
#[must_use]
pub fn active_weathers_at(world: &World, observer_position: Vec3) -> Vec<WeatherSpec> {
    let all = extract_all_weathers(world);
    let volume_matches: Vec<WeatherSpec> = all
        .iter()
        .filter(|item| item.is_volume && item.contains_point(observer_position))
        .map(|item| item.spec.clone())
        .collect();
    if !volume_matches.is_empty() {
        return volume_matches;
    }
    all.into_iter()
        .filter(|item| !item.is_volume)
        .map(|item| item.spec)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::TableSpec;
    use crate::value::ValueKind;

    #[test]
    fn weather_volume_overrides_global() {
        let mut world = World::new(42);
        world
            .define_table(
                TableSpec::new(WEATHER_TABLE)
                    .column("preset", ValueKind::Text, Value::Text("clear".to_string()))
                    .column("hours", ValueKind::Float, Value::Float(13.0))
                    .column("weight", ValueKind::Float, Value::Float(1.0))
                    .column("volume", ValueKind::Text, Value::Text(String::new())),
            )
            .expect("define");
        world
            .define_table(
                TableSpec::new("Transform")
                    .column("position", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("rotation_degrees", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("scale", ValueKind::Vec3, Value::Vec3(Vec3::ONE))
                    .column("pivot", ValueKind::Vec3, Value::Vec3(Vec3::ZERO))
                    .column("parent", ValueKind::Int, Value::Int(0)),
            )
            .expect("define");

        // Global weather: clear, 12h
        let global_handle = world.spawn().expect("spawn");
        world
            .insert_row_values(
                global_handle,
                WEATHER_TABLE,
                vec![
                    Value::Text("clear".to_string()),
                    Value::Float(12.0),
                    Value::Float(1.0),
                    Value::Text(String::new()),
                ],
            )
            .expect("insert");

        // Box weather at (10, 0, 0) of size (4, 4, 4): rain, 21h
        let box_handle = world.spawn().expect("spawn");
        world
            .insert_row_values(
                box_handle,
                "Transform",
                vec![
                    Value::Vec3(Vec3::new(10.0, 0.0, 0.0)),
                    Value::Vec3(Vec3::ZERO),
                    Value::Vec3(Vec3::new(4.0, 4.0, 4.0)),
                    Value::Vec3(Vec3::ZERO),
                    Value::Int(0),
                ],
            )
            .expect("insert");
        world
            .insert_row_values(
                box_handle,
                WEATHER_TABLE,
                vec![
                    Value::Text("rain".to_string()),
                    Value::Float(21.0),
                    Value::Float(0.8),
                    Value::Text("box".to_string()),
                ],
            )
            .expect("insert");

        // Observer outside box volume sees global weather
        let outside = active_weathers_at(&world, Vec3::ZERO);
        assert_eq!(outside.len(), 1);
        assert_eq!(outside[0].preset, "clear");

        // Observer inside box volume sees box weather
        let inside = active_weathers_at(&world, Vec3::new(10.5, 0.2, 0.1));
        assert_eq!(inside.len(), 1);
        assert_eq!(inside[0].preset, "rain");
        assert_eq!(inside[0].hours, 21.0);
    }
}

//! SkyLayer table and specifications (DR-53 Frente 7).
//!
//! A scene can have multiple `SkyLayer` entities drawn in order of their `order` column.
//! Each layer can be a solid color or an equirectangular image from `assets/` with animated
//! scrolling based on scene time.

use petunia_math::{Color, Vec3};
use serde::{Deserialize, Serialize};

use crate::world::World;
use crate::{EntityHandle, Value};

/// Table name for sky layer components.
pub const SKY_LAYER_TABLE: &str = "SkyLayer";

/// A sky layer specification from an entity.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkyLayerSpec {
    pub order: i64,
    pub color: Color,
    pub image: String,
    pub alpha: f32,
    pub scroll: Vec3,
}

impl Default for SkyLayerSpec {
    fn default() -> Self {
        Self {
            order: 0,
            color: Color::WHITE,
            image: String::new(),
            alpha: 1.0,
            scroll: Vec3::ZERO,
        }
    }
}

/// Reads the sky layer specification of an entity, if it has a `SkyLayer` row.
#[must_use]
pub fn sky_layer_of(world: &World, handle: EntityHandle) -> Option<SkyLayerSpec> {
    if !world.has_row(handle, SKY_LAYER_TABLE) {
        return None;
    }
    let order = match world.get(handle, SKY_LAYER_TABLE, "order") {
        Ok(Value::Int(value)) => *value,
        _ => 0,
    };
    let color = match world.get(handle, SKY_LAYER_TABLE, "color") {
        Ok(Value::Color(value)) => *value,
        _ => Color::WHITE,
    };
    let image = match world.get(handle, SKY_LAYER_TABLE, "image") {
        Ok(Value::Text(text)) => text.clone(),
        _ => String::new(),
    };
    let alpha = match world.get(handle, SKY_LAYER_TABLE, "alpha") {
        Ok(Value::Float(value)) if value.is_finite() => (*value as f32).clamp(0.0, 1.0),
        _ => 1.0,
    };
    let scroll = match world.get(handle, SKY_LAYER_TABLE, "scroll") {
        Ok(Value::Vec3(value))
            if value.x.is_finite() && value.y.is_finite() && value.z.is_finite() =>
        {
            *value
        }
        _ => Vec3::ZERO,
    };
    Some(SkyLayerSpec {
        order,
        color,
        image,
        alpha,
        scroll,
    })
}

/// Extracts all sky layers from the world, sorted by `order` ascending.
#[must_use]
pub fn extract_sky_layers(world: &World) -> Vec<SkyLayerSpec> {
    let Ok(handles) = world.entities_with_table(SKY_LAYER_TABLE) else {
        return Vec::new();
    };
    let mut layers: Vec<SkyLayerSpec> = handles
        .into_iter()
        .filter_map(|handle| sky_layer_of(world, handle))
        .collect();
    layers.sort_by_key(|layer| layer.order);
    layers
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::TableSpec;
    use crate::value::ValueKind;

    #[test]
    fn sky_layers_extract_in_order() {
        let mut world = World::new(42);
        world
            .define_table(
                TableSpec::new(SKY_LAYER_TABLE)
                    .column("order", ValueKind::Int, Value::Int(0))
                    .column("color", ValueKind::Color, Value::Color(Color::WHITE))
                    .column("image", ValueKind::Text, Value::Text(String::new()))
                    .column("alpha", ValueKind::Float, Value::Float(1.0))
                    .column("scroll", ValueKind::Vec3, Value::Vec3(Vec3::ZERO)),
            )
            .expect("define");

        let handle1 = world.spawn().expect("spawn");
        world
            .insert_row_values(
                handle1,
                SKY_LAYER_TABLE,
                vec![
                    Value::Int(10),
                    Value::Color(Color::WHITE),
                    Value::Text("assets/clouds.png".to_string()),
                    Value::Float(0.5),
                    Value::Vec3(Vec3::ZERO),
                ],
            )
            .expect("insert");

        let handle0 = world.spawn().expect("spawn");
        world
            .insert_row_values(
                handle0,
                SKY_LAYER_TABLE,
                vec![
                    Value::Int(-5),
                    Value::Color(Color::rgb(0.1, 0.2, 0.5)),
                    Value::Text(String::new()),
                    Value::Float(1.0),
                    Value::Vec3(Vec3::ZERO),
                ],
            )
            .expect("insert");

        let layers = extract_sky_layers(&world);
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].order, -5);
        assert_eq!(layers[1].order, 10);
        assert_eq!(layers[1].image, "assets/clouds.png");
    }
}

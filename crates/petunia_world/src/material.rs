//! Material definitions for objects and faces (DR-53).
//!
//! A material is plain data in the `Material` table. Five kinds are supported without a node graph:
//! `flat`, `standard`, `masked`, `blended`, `post`. An object without a `Material` row keeps its
//! brush or mesh colour.

use crate::ids::EntityHandle;
use crate::value::Value;
use crate::world::World;
use petunia_math::Color;

/// Table name for materials.
pub const MATERIAL_TABLE: &str = "Material";

/// The five kinds of materials supported by the engine (DR-53).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum MaterialKind {
    /// Color without lighting calculations (unlit).
    Flat,
    /// Diffuse lighting with metallic, roughness and optional textures.
    #[default]
    Standard,
    /// Standard material with alpha testing (pixels below cutoff are discarded).
    Masked,
    /// Standard material with alpha blending (drawn in translucent pass).
    Blended,
    /// Full-screen post-processing effect pass (e.g. grade, fog).
    Post,
}

impl MaterialKind {
    pub fn parse(text: &str) -> Self {
        match text {
            "flat" => Self::Flat,
            "masked" => Self::Masked,
            "blended" => Self::Blended,
            "post" => Self::Post,
            _ => Self::Standard,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Flat => "flat",
            Self::Standard => "standard",
            Self::Masked => "masked",
            Self::Blended => "blended",
            Self::Post => "post",
        }
    }
}

/// Material properties read from the `Material` table of an entity.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialSpec {
    pub kind: MaterialKind,
    pub color: Color,
    pub alpha: f32,
    pub metallic: f32,
    pub roughness: f32,
    pub cutoff: f32,
    pub base_texture: String,
    pub normal_texture: String,
    pub effect: String,
}

impl Default for MaterialSpec {
    fn default() -> Self {
        Self {
            kind: MaterialKind::Standard,
            color: Color::rgb(0.8, 0.8, 0.8),
            alpha: 1.0,
            metallic: 0.0,
            roughness: 0.5,
            cutoff: 0.5,
            base_texture: String::new(),
            normal_texture: String::new(),
            effect: String::new(),
        }
    }
}

/// Reads the material attached directly to `handle`, if present.
pub fn material_of(world: &World, handle: EntityHandle) -> Option<MaterialSpec> {
    if !world.has_row(handle, MATERIAL_TABLE) {
        return None;
    }
    let kind = match world.get(handle, MATERIAL_TABLE, "kind") {
        Ok(Value::Text(text)) => MaterialKind::parse(text),
        _ => MaterialKind::Standard,
    };
    let color = match world.get(handle, MATERIAL_TABLE, "color") {
        Ok(Value::Color(color)) => *color,
        _ => Color::rgb(0.8, 0.8, 0.8),
    };
    let alpha = match world.get(handle, MATERIAL_TABLE, "alpha") {
        Ok(Value::Float(alpha)) => (*alpha as f32).clamp(0.0, 1.0),
        _ => 1.0,
    };
    let metallic = match world.get(handle, MATERIAL_TABLE, "metallic") {
        Ok(Value::Float(val)) => (*val as f32).clamp(0.0, 1.0),
        _ => 0.0,
    };
    let roughness = match world.get(handle, MATERIAL_TABLE, "roughness") {
        Ok(Value::Float(val)) => (*val as f32).clamp(0.0, 1.0),
        _ => 0.5,
    };
    let cutoff = match world.get(handle, MATERIAL_TABLE, "cutoff") {
        Ok(Value::Float(val)) => (*val as f32).clamp(0.0, 1.0),
        _ => 0.5,
    };
    let base_texture = match world.get(handle, MATERIAL_TABLE, "base_texture") {
        Ok(Value::Text(text)) => text.clone(),
        _ => String::new(),
    };
    let normal_texture = match world.get(handle, MATERIAL_TABLE, "normal_texture") {
        Ok(Value::Text(text)) => text.clone(),
        _ => String::new(),
    };
    let effect = match world.get(handle, MATERIAL_TABLE, "effect") {
        Ok(Value::Text(text)) => text.clone(),
        _ => String::new(),
    };

    Some(MaterialSpec {
        kind,
        color,
        alpha,
        metallic,
        roughness,
        cutoff,
        base_texture,
        normal_texture,
        effect,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::TableSpec;
    use crate::value::ValueKind;

    #[test]
    fn material_round_trips_all_five_kinds() {
        let mut world = World::new(7);
        world
            .define_table(
                TableSpec::new(MATERIAL_TABLE)
                    .column("kind", ValueKind::Text, Value::Text("standard".to_string()))
                    .column("color", ValueKind::Color, Value::Color(Color::WHITE))
                    .column("alpha", ValueKind::Float, Value::Float(1.0))
                    .column("metallic", ValueKind::Float, Value::Float(0.0))
                    .column("roughness", ValueKind::Float, Value::Float(0.5))
                    .column("cutoff", ValueKind::Float, Value::Float(0.5))
                    .column("base_texture", ValueKind::Text, Value::Text(String::new()))
                    .column(
                        "normal_texture",
                        ValueKind::Text,
                        Value::Text(String::new()),
                    )
                    .column("effect", ValueKind::Text, Value::Text(String::new())),
            )
            .expect("define");

        let handle = world.spawn().expect("spawn");
        assert_eq!(material_of(&world, handle), None);

        world
            .insert_row_json(handle, MATERIAL_TABLE, &serde_json::Map::new())
            .expect("insert row");
        let default_spec = material_of(&world, handle).expect("default material");
        assert_eq!(default_spec.kind, MaterialKind::Standard);

        world
            .set(
                handle,
                MATERIAL_TABLE,
                "kind",
                Value::Text("flat".to_string()),
            )
            .expect("set");
        let spec = material_of(&world, handle).expect("material");
        assert_eq!(spec.kind, MaterialKind::Flat);
        assert_eq!(spec.color, Color::WHITE);

        for (kind_str, kind_enum) in [
            ("flat", MaterialKind::Flat),
            ("standard", MaterialKind::Standard),
            ("masked", MaterialKind::Masked),
            ("blended", MaterialKind::Blended),
            ("post", MaterialKind::Post),
        ] {
            assert_eq!(MaterialKind::parse(kind_str), kind_enum);
            assert_eq!(kind_enum.as_str(), kind_str);
        }
    }
}

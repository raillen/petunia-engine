use petunia_math::{Color, PolygonMesh, Vec3};
use petunia_terrain::{ChunkCoord, Terrain, TerrainChunk, TerrainHole};
use serde::{Deserialize, Serialize};

/// Largest integer that every language in the engine can represent exactly (2^53 - 1).
/// Aipo's `Int` uses the same safe range.
pub const MAX_SAFE_INT: i64 = 9_007_199_254_740_991;

/// Default ceiling for a text value, in bytes.
pub const MAX_TEXT_BYTES: usize = 1024;

/// The type of a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Bool,
    Int,
    Float,
    Text,
    Vec3,
    Color,
    /// An editable polygon mesh (see `petunia_math::PolygonMesh`).
    Mesh,
    /// A chunked heightmap (see `petunia_terrain::Terrain`).
    Terrain,
}

impl ValueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::Int => "int",
            Self::Float => "float",
            Self::Text => "text",
            Self::Vec3 => "vec3",
            Self::Color => "color",
            Self::Mesh => "mesh",
            Self::Terrain => "terrain",
        }
    }
}

/// One cell. Values copy by value; there are no hidden references.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Vec3(Vec3),
    Color(Color),
    Mesh(PolygonMesh),
    Terrain(Terrain),
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::Bool(_) => ValueKind::Bool,
            Self::Int(_) => ValueKind::Int,
            Self::Float(_) => ValueKind::Float,
            Self::Text(_) => ValueKind::Text,
            Self::Vec3(_) => ValueKind::Vec3,
            Self::Color(_) => ValueKind::Color,
            Self::Mesh(_) => ValueKind::Mesh,
            Self::Terrain(_) => ValueKind::Terrain,
        }
    }

    /// Checks the value against the hostile-data rules: finite floats, safe integers, bounded
    /// text without NUL. Returns the reason on failure.
    pub fn validate(&self, max_text_bytes: usize) -> Result<(), String> {
        match self {
            Self::Bool(_) => Ok(()),
            Self::Int(value) => {
                if (-MAX_SAFE_INT..=MAX_SAFE_INT).contains(value) {
                    Ok(())
                } else {
                    Err("inteiro fora do intervalo seguro (de -2^53+1 a 2^53-1)".to_string())
                }
            }
            Self::Float(value) => finite(value.is_finite()),
            Self::Text(text) => {
                if text.len() > max_text_bytes {
                    Err(format!("texto maior que {max_text_bytes} bytes"))
                } else if text.contains('\0') {
                    Err("o texto contém o caractere NUL".to_string())
                } else {
                    Ok(())
                }
            }
            Self::Vec3(vector) => finite(vector.is_finite()),
            Self::Color(color) => finite(color.is_finite()),
            Self::Mesh(mesh) => mesh.validate().map_err(|error| error.to_string()),
            Self::Terrain(terrain) => terrain.validate().map_err(|error| error.to_string()),
        }
    }

    /// Reads a plain JSON value as the given kind and validates it.
    pub fn from_json(
        kind: ValueKind,
        json: &serde_json::Value,
        max_text_bytes: usize,
    ) -> Result<Self, String> {
        let value = match kind {
            ValueKind::Bool => Self::Bool(json.as_bool().ok_or("esperava verdadeiro ou falso")?),
            ValueKind::Int => Self::Int(json.as_i64().ok_or("esperava um número inteiro")?),
            ValueKind::Float => Self::Float(json.as_f64().ok_or("esperava um número")?),
            ValueKind::Text => Self::Text(json.as_str().ok_or("esperava um texto")?.to_string()),
            ValueKind::Vec3 => Self::Vec3(
                serde_json::from_value::<Vec3>(json.clone())
                    .map_err(|error| format!("esperava {{x, y, z}}: {error}"))?,
            ),
            ValueKind::Color => Self::Color(
                serde_json::from_value::<Color>(json.clone())
                    .map_err(|error| format!("esperava {{r, g, b, a}}: {error}"))?,
            ),
            ValueKind::Mesh => {
                Self::Mesh(PolygonMesh::from_text_json(json).map_err(|error| error.to_string())?)
            }
            ValueKind::Terrain => Self::Terrain(terrain_from_json(json)?),
        };
        value.validate(max_text_bytes)?;
        Ok(value)
    }

    /// Plain JSON form (no type tags: the column's kind says how to read it back).
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Bool(value) => serde_json::Value::Bool(*value),
            Self::Int(value) => serde_json::Value::from(*value),
            Self::Float(value) => serde_json::Number::from_f64(*value)
                .map_or(serde_json::Value::Null, serde_json::Value::Number),
            Self::Text(text) => serde_json::Value::String(text.clone()),
            Self::Vec3(vector) => serde_json::json!({
                "x": f32_json(vector.x),
                "y": f32_json(vector.y),
                "z": f32_json(vector.z),
            }),
            Self::Color(color) => serde_json::json!({
                "r": f32_json(color.r),
                "g": f32_json(color.g),
                "b": f32_json(color.b),
                "a": f32_json(color.a),
            }),
            Self::Mesh(mesh) => mesh.to_text_json(),
            Self::Terrain(terrain) => terrain_to_json(terrain),
        }
    }
}

fn terrain_to_json(terrain: &Terrain) -> serde_json::Value {
    let chunks: Vec<serde_json::Value> = terrain
        .chunks()
        .map(|chunk| {
            let coord = chunk.coord();
            let heights: Vec<serde_json::Value> = chunk
                .heights()
                .iter()
                .map(|height| f32_json(*height))
                .collect();
            let materials: Vec<serde_json::Value> = chunk
                .materials()
                .iter()
                .map(|material| serde_json::Value::from(*material))
                .collect();
            serde_json::json!({
                "x": coord.x,
                "z": coord.z,
                "heights": heights,
                "materials": materials,
            })
        })
        .collect();
    let holes: Vec<serde_json::Value> = terrain
        .holes()
        .iter()
        .map(|hole| {
            serde_json::json!({
                "x": f32_json(hole.x),
                "z": f32_json(hole.z),
                "radius": f32_json(hole.radius),
            })
        })
        .collect();
    serde_json::json!({
        "cell_size": f32_json(terrain.cell_size()),
        "chunk_cells": terrain.chunk_cells(),
        "chunks": chunks,
        "holes": holes,
    })
}

fn terrain_from_json(json: &serde_json::Value) -> Result<Terrain, String> {
    let object = json
        .as_object()
        .ok_or_else(|| "esperava um objeto de terreno".to_string())?;
    let cell_size = object
        .get("cell_size")
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| "o terreno precisa de `cell_size`".to_string())? as f32;
    let chunk_cells = object
        .get("chunk_cells")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "o terreno precisa de `chunk_cells`".to_string())?;
    let chunk_cells = usize::try_from(chunk_cells)
        .map_err(|_| "`chunk_cells` não cabe na memória".to_string())?;
    let chunks = object
        .get("chunks")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "o terreno precisa de `chunks`".to_string())?;
    let expected = chunk_cells
        .checked_mul(chunk_cells)
        .ok_or_else(|| "`chunk_cells` grande demais".to_string())?;
    let mut terrain = Terrain::new(cell_size, chunk_cells).map_err(|error| error.to_string())?;
    for chunk_json in chunks {
        let chunk = chunk_json
            .as_object()
            .ok_or_else(|| "esperava um objeto de bloco".to_string())?;
        let x = chunk
            .get("x")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| "o bloco precisa de `x` inteiro".to_string())?;
        let z = chunk
            .get("z")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .ok_or_else(|| "o bloco precisa de `z` inteiro".to_string())?;
        let heights_json = chunk
            .get("heights")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "o bloco precisa de `heights`".to_string())?;
        if heights_json.len() != expected {
            return Err("`heights` não tem uma entrada por célula".to_string());
        }
        let mut heights = Vec::with_capacity(expected);
        for height in heights_json {
            heights.push(
                height
                    .as_f64()
                    .ok_or_else(|| "cada altura precisa ser um número".to_string())?
                    as f32,
            );
        }
        let materials = match chunk.get("materials") {
            None | Some(serde_json::Value::Null) => Vec::new(),
            Some(serde_json::Value::Array(values)) => {
                if values.len() != expected {
                    return Err("`materials` não tem uma entrada por célula".to_string());
                }
                let mut materials = Vec::with_capacity(expected);
                for material in values {
                    let value = material
                        .as_u64()
                        .ok_or_else(|| "cada material precisa ser um inteiro".to_string())?;
                    materials.push(
                        u8::try_from(value)
                            .map_err(|_| "o material precisa estar entre 0 e 255".to_string())?,
                    );
                }
                materials
            }
            Some(_) => return Err("`materials` precisa ser uma lista".to_string()),
        };
        let chunk =
            TerrainChunk::from_parts(ChunkCoord::new(x, z), chunk_cells, heights, materials)
                .map_err(|error| error.to_string())?;
        terrain
            .insert_chunk(chunk)
            .map_err(|error| error.to_string())?;
    }
    let holes = match object.get("holes") {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::Array(values)) => {
            let mut holes = Vec::with_capacity(values.len());
            for value in values {
                let hole = value
                    .as_object()
                    .ok_or_else(|| "cada furo precisa ser um objeto".to_string())?;
                let number = |key: &str| -> Result<f32, String> {
                    hole.get(key)
                        .and_then(serde_json::Value::as_f64)
                        .map(|value| value as f32)
                        .ok_or_else(|| format!("o furo precisa de `{key}`"))
                };
                holes.push(TerrainHole {
                    x: number("x")?,
                    z: number("z")?,
                    radius: number("radius")?,
                });
            }
            holes
        }
        Some(_) => return Err("`holes` precisa ser uma lista".to_string()),
    };
    terrain
        .set_holes(holes)
        .map_err(|error| error.to_string())?;
    terrain.validate().map_err(|error| error.to_string())?;
    Ok(terrain)
}

/// An `f32` as JSON using the shortest decimal that reads back as the same `f32`: `0.62`, not
/// `0.6200000047683716` (what widening to `f64` would print). Keeps scene files clean in diffs.
fn f32_json(value: f32) -> serde_json::Value {
    value
        .to_string()
        .parse::<f64>()
        .ok()
        .and_then(serde_json::Number::from_f64)
        .map_or(serde_json::Value::Null, serde_json::Value::Number)
}

fn finite(is_finite: bool) -> Result<(), String> {
    if is_finite {
        Ok(())
    } else {
        Err("o valor não é um número finito".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_values_serialize_short_and_read_back_exactly() {
        let color = Color::rgb(0.62, 0.58, 0.55);
        let text = Value::Color(color).to_json().to_string();
        assert!(
            text.contains("0.62") && !text.contains("0.6200000"),
            "{text}"
        );
        let back = Value::from_json(ValueKind::Color, &Value::Color(color).to_json(), 16);
        assert_eq!(back, Ok(Value::Color(color)));

        let vector = Vec3::new(0.1, -2.5, 1.0e-3);
        let text = Value::Vec3(vector).to_json().to_string();
        assert!(text.contains("0.1") && text.contains("0.001"), "{text}");
        let back = Value::from_json(ValueKind::Vec3, &Value::Vec3(vector).to_json(), 16);
        assert_eq!(back, Ok(Value::Vec3(vector)));
    }

    #[test]
    fn extreme_integers_are_refused_without_panicking() {
        for extreme in [i64::MIN, i64::MAX, MAX_SAFE_INT + 1, -MAX_SAFE_INT - 1] {
            let json = serde_json::json!(extreme);
            assert!(
                Value::from_json(ValueKind::Int, &json, 64).is_err(),
                "{extreme} must be outside the safe range"
            );
        }
        for safe in [MAX_SAFE_INT, -MAX_SAFE_INT, 0] {
            let json = serde_json::json!(safe);
            assert_eq!(
                Value::from_json(ValueKind::Int, &json, 64),
                Ok(Value::Int(safe))
            );
        }
    }

    #[test]
    fn meshes_round_trip_and_refuse_broken_geometry() {
        let mesh = PolygonMesh {
            positions: vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.5, 0.0, 0.0),
                Vec3::new(0.0, 0.25, 0.0),
            ],
            faces: vec![vec![0, 1, 2]],
        };
        let json = Value::Mesh(mesh.clone()).to_json();
        assert_eq!(json["positions"][1], "1.5 0 0");
        assert_eq!(
            Value::from_json(ValueKind::Mesh, &json, 16),
            Ok(Value::Mesh(mesh))
        );
        let broken = serde_json::json!({ "positions": ["0 0 0"], "faces": ["0 1 2"] });
        assert!(Value::from_json(ValueKind::Mesh, &broken, 16).is_err());
    }
}

#[cfg(test)]
mod terrain_tests {
    use super::*;

    fn sample_terrain() -> Terrain {
        let mut terrain = Terrain::new(1.0, 4).unwrap();
        terrain.set_sample(0, 0, 1.5).unwrap();
        terrain.set_sample_material(0, 0, 2).unwrap();
        terrain.set_sample(4, 4, -2.0).unwrap();
        terrain
            .set_holes(vec![TerrainHole {
                x: 1.0,
                z: 2.0,
                radius: 0.75,
            }])
            .unwrap();
        terrain
    }

    #[test]
    fn terrain_round_trips_through_json() {
        let terrain = sample_terrain();
        let json = Value::Terrain(terrain.clone()).to_json();
        let back = Value::from_json(ValueKind::Terrain, &json, 16);
        assert_eq!(back, Ok(Value::Terrain(terrain)));
    }

    #[test]
    fn hostile_terrain_json_is_refused() {
        let short = serde_json::json!({
            "cell_size": 1.0,
            "chunk_cells": 4,
            "chunks": [{ "x": 0, "z": 0, "heights": vec![0.0; 15] }],
        });
        assert!(Value::from_json(ValueKind::Terrain, &short, 16).is_err());

        let not_numbers = serde_json::json!({
            "cell_size": 1.0,
            "chunk_cells": 4,
            "chunks": [{ "x": 0, "z": 0, "heights": vec![serde_json::Value::Null; 16] }],
        });
        assert!(Value::from_json(ValueKind::Terrain, &not_numbers, 16).is_err());

        let bad_cell = serde_json::json!({
            "cell_size": 0.0,
            "chunk_cells": 4,
            "chunks": [],
        });
        assert!(Value::from_json(ValueKind::Terrain, &bad_cell, 16).is_err());
    }
}

use petunia_math::{Color, PolygonMesh, Vec3};
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
        }
    }

    /// Checks the value against the hostile-data rules: finite floats, safe integers, bounded
    /// text without NUL. Returns the reason on failure.
    pub fn validate(&self, max_text_bytes: usize) -> Result<(), String> {
        match self {
            Self::Bool(_) => Ok(()),
            Self::Int(value) => {
                if value.abs() <= MAX_SAFE_INT {
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
        }
    }
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

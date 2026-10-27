//! Values between a script and the world. The world speaks JSON (the same form commands, save files
//! and the network use); a script speaks Aipo values. Numbers keep their kind: a whole number that
//! fits Aipo's safe range is an `Int`, anything else a `Float`.

use std::cell::RefCell;
use std::rc::Rc;

use aipo_vm::{DictMap, Value};
use serde_json::{Map, Number, Value as Json};

/// Aipo's `Int` is exact up to this magnitude (2^53 - 1).
const MAX_SAFE_INT: i64 = (1 << 53) - 1;
/// Nesting deeper than this is refused: a script cannot build a value that overflows the stack.
const MAX_DEPTH: usize = 16;

pub(crate) fn json_to_value(json: &Json) -> Value {
    match json {
        Json::Null => Value::None,
        Json::Bool(flag) => Value::Bool(*flag),
        Json::Number(number) => match number.as_i64() {
            Some(whole) if (-MAX_SAFE_INT..=MAX_SAFE_INT).contains(&whole) => Value::Int(whole),
            _ => Value::Float(number.as_f64().unwrap_or(0.0)),
        },
        Json::String(text) => Value::string(text.clone()),
        Json::Array(items) => Value::list(items.iter().map(json_to_value).collect::<Vec<_>>()),
        Json::Object(fields) => {
            let entries = fields
                .iter()
                .map(|(key, value)| (Value::string(key.clone()), json_to_value(value)))
                .collect();
            Value::Dict(Rc::new(RefCell::new(DictMap::from_entries(entries))))
        }
    }
}

/// A value for the world, or the reason it cannot go there.
pub(crate) fn value_to_json(value: &Value) -> Result<Json, String> {
    to_json(value, 0)
}

fn to_json(value: &Value, depth: usize) -> Result<Json, String> {
    if depth > MAX_DEPTH {
        return Err("valor aninhado demais".to_string());
    }
    Ok(match value {
        Value::None => Json::Null,
        Value::Bool(flag) => Json::Bool(*flag),
        Value::Int(whole) => Json::Number(Number::from(*whole)),
        Value::Byte(byte) => Json::Number(Number::from(*byte)),
        Value::Float(real) => Json::Number(
            Number::from_f64(*real).ok_or_else(|| "número que não é finito".to_string())?,
        ),
        Value::String(text) => Json::String(text.to_string()),
        Value::List(items) => Json::Array(
            items
                .borrow()
                .iter()
                .map(|item| to_json(item, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Value::Dict(entries) => {
            let mut fields = Map::new();
            for (key, item) in entries.borrow().entries() {
                let Value::String(name) = key else {
                    return Err(
                        "as chaves de um dicionário para o mundo precisam ser texto".to_string()
                    );
                };
                fields.insert(name.to_string(), to_json(item, depth + 1)?);
            }
            Json::Object(fields)
        }
        other => {
            return Err(format!(
                "um valor `{}` não pode ir para o mundo",
                other.type_name()
            ));
        }
    })
}

/// How a value reads in the console.
pub(crate) fn display_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_string(),
        Value::List(_) | Value::Dict(_) => value_to_json(value)
            .map(|json| json.to_string())
            .unwrap_or_else(|_| value.to_string()),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_round_trips_through_values() {
        let original = json!({ "x": 1.5, "y": 2, "tags": ["a", true, null], "name": "caixa" });
        let value = json_to_value(&original);
        assert_eq!(value_to_json(&value), Ok(original));
    }

    #[test]
    fn whole_numbers_are_ints_and_the_rest_floats() {
        assert!(matches!(json_to_value(&json!(7)), Value::Int(7)));
        assert!(matches!(json_to_value(&json!(7.5)), Value::Float(_)));
        assert!(matches!(json_to_value(&json!(1u64 << 60)), Value::Float(_)));
    }

    #[test]
    fn the_extreme_integer_becomes_a_float_instead_of_panicking() {
        assert!(matches!(json_to_value(&json!(i64::MIN)), Value::Float(_)));
        assert!(matches!(json_to_value(&json!(i64::MAX)), Value::Float(_)));
    }

    #[test]
    fn values_that_cannot_go_to_the_world_are_refused_not_dropped() {
        let deep = (0..40).fold(json!(1), |inner, _| json!([inner]));
        assert!(value_to_json(&json_to_value(&deep)).is_err());
        let native = Value::native("x", 0, |_| Ok(Value::None));
        assert!(value_to_json(&native).is_err());
    }
}

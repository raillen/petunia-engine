use petunia_world::{MAX_SAFE_INT, MAX_TEXT_BYTES, Value, ValueKind};
use serde_json::Value as Json;

use crate::spec::{ParamKind, ParamSpec};

/// Checks a JSON payload against a list of parameters: it must be an object, required
/// parameters must be present, unknown names are rejected, and every value must match its kind
/// and range. Returns a message that says what to fix.
pub fn check_params(params: &[ParamSpec], payload: &Json) -> Result<(), String> {
    let empty = serde_json::Map::new();
    let object = match payload {
        Json::Object(map) => map,
        Json::Null => &empty,
        _ => return Err("o payload precisa ser um objeto JSON".to_string()),
    };
    if let Some(unknown) = object
        .keys()
        .find(|key| !params.iter().any(|param| &param.name == *key))
    {
        return Err(format!("parâmetro desconhecido `{unknown}`"));
    }
    for param in params {
        match object.get(&param.name) {
            None | Some(Json::Null) if param.required => {
                return Err(format!("falta o parâmetro `{}`", param.name));
            }
            None | Some(Json::Null) => {}
            Some(value) => check_one(param, value)
                .map_err(|reason| format!("parâmetro `{}`: {reason}", param.name))?,
        }
    }
    Ok(())
}

fn check_one(param: &ParamSpec, value: &Json) -> Result<(), String> {
    match param.kind {
        ParamKind::Bool => scalar(ValueKind::Bool, value)?,
        ParamKind::Int => scalar(ValueKind::Int, value)?,
        ParamKind::Float => scalar(ValueKind::Float, value)?,
        ParamKind::Text => scalar(ValueKind::Text, value)?,
        ParamKind::Vec3 => scalar(ValueKind::Vec3, value)?,
        ParamKind::Color => scalar(ValueKind::Color, value)?,
        ParamKind::Entity => {
            let id = value
                .as_i64()
                .ok_or("esperava um id de entidade (inteiro positivo)")?;
            if !(1..=MAX_SAFE_INT).contains(&id) {
                return Err("id de entidade fora do intervalo".to_string());
            }
        }
        ParamKind::TextList => {
            let items = value.as_array().ok_or("esperava uma lista de textos")?;
            if items.iter().any(|item| !item.is_string()) {
                return Err("a lista só pode conter textos".to_string());
            }
        }
        ParamKind::Rows => {
            let tables = value
                .as_object()
                .ok_or("esperava um objeto {Tabela: {coluna: valor}}")?;
            if tables.values().any(|row| !row.is_object()) {
                return Err("cada tabela precisa de um objeto de colunas".to_string());
            }
        }
        ParamKind::Any => {}
    }
    if let Some(number) = value.as_f64() {
        if let Some(minimum) = param.minimum
            && number < minimum
        {
            return Err(format!("valor {number} abaixo do mínimo {minimum}"));
        }
        if let Some(maximum) = param.maximum
            && number > maximum
        {
            return Err(format!("valor {number} acima do máximo {maximum}"));
        }
    }
    Ok(())
}

fn scalar(kind: ValueKind, value: &Json) -> Result<(), String> {
    Value::from_json(kind, value, MAX_TEXT_BYTES).map(|_| ())
}

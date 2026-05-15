use crate::dynamic_schema::{CompiledSchema, FieldType};
use serde::Deserialize;
use serde_json::{Deserializer, Value};

pub fn stream_parse_array(input: &[u8], schema: &CompiledSchema) -> Result<Vec<Value>, String> {
    let mut deserializer = Deserializer::from_slice(input);
    let values: Value = Value::deserialize(&mut deserializer).map_err(|e| e.to_string())?;

    match values {
        Value::Array(arr) => {
            let mut validated = Vec::with_capacity(arr.len());
            for v in arr {
                validated.push(validate_and_rebuild(v, schema)?);
            }
            Ok(validated)
        }
        _ => Err("Expected array of objects".to_string()),
    }
}

pub fn stream_parse_one(input: &[u8], schema: &CompiledSchema) -> Result<Value, String> {
    let mut deserializer = Deserializer::from_slice(input);
    let value: Value = Value::deserialize(&mut deserializer).map_err(|e| e.to_string())?;
    validate_and_rebuild(value, schema)
}

fn validate_and_rebuild(v: Value, schema: &CompiledSchema) -> Result<Value, String> {
    match v {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(schema.fields.len());
            for field in &schema.fields {
                match map.get(&field.name) {
                    Some(val) => {
                        validate_type(val, field)?;
                        out.insert(field.name.clone(), val.clone());
                    }
                    None => {
                        if field.optional {
                            out.insert(field.name.clone(), Value::Null);
                        } else {
                            return Err(format!("Missing required field: {}", field.name));
                        }
                    }
                }
            }
            Ok(Value::Object(out))
        }
        _ => Err("Expected object".to_string()),
    }
}

fn validate_type(val: &Value, field: &crate::dynamic_schema::CompiledField) -> Result<(), String> {
    let ok = match (&field.type_, val) {
        (FieldType::String, Value::String(_)) => true,
        (FieldType::I64, Value::Number(n)) => n.is_i64(),
        (FieldType::F64, Value::Number(_)) => true,
        (FieldType::Bool, Value::Bool(_)) => true,
        (FieldType::Json, _) => true,
        (FieldType::ArrayString, Value::Array(arr)) => arr.iter().all(|v| v.is_string()),
        (FieldType::ArrayI64, Value::Array(arr)) => arr.iter().all(|v| v.is_i64()),
        (FieldType::ArrayF64, Value::Array(arr)) => arr.iter().all(|v| v.is_number()),
        _ => false,
    };

    if ok || (field.optional && val.is_null()) {
        Ok(())
    } else {
        Err(format!("Invalid type for field: {}", field.name))
    }
}

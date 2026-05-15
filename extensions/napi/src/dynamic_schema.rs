#![allow(clippy::redundant_closure)]
use napi::bindgen_prelude::*;
use napi_derive::napi;
use rustc_hash::FxHashMap;
use serde_json::Value;

#[napi(string_enum)]
#[derive(Clone, Debug)]
pub enum FieldType {
  #[napi(value = "string")]
  String,
  #[napi(value = "i64")]
  I64,
  #[napi(value = "f64")]
  F64,
  #[napi(value = "bool")]
  Bool,
  #[napi(value = "json")]
  Json,
  #[napi(value = "array:string")]
  ArrayString,
  #[napi(value = "array:i64")]
  ArrayI64,
  #[napi(value = "array:f64")]
  ArrayF64,
}

#[napi(object)]
pub struct SchemaField {
  pub name: String,
  pub type_: FieldType,
  pub optional: Option<bool>,
}

pub struct CompiledField {
  pub name: String,
  pub type_: FieldType,
  pub optional: bool,
}

pub struct CompiledSchema {
  pub fields: Vec<CompiledField>,
  pub indices: FxHashMap<String, usize>,
}

fn type_label(t: &FieldType) -> &'static str {
  match t {
    FieldType::String => "string",
    FieldType::I64 => "i64",
    FieldType::F64 => "f64",
    FieldType::Bool => "bool",
    FieldType::Json => "json",
    FieldType::ArrayString => "array<string>",
    FieldType::ArrayI64 => "array<i64>",
    FieldType::ArrayF64 => "array<f64>",
  }
}

fn validate_value(val: &Value, field: &CompiledField) -> Result<()> {
  let ok = match (&field.type_, val) {
    (FieldType::String, Value::String(_)) => true,
    (FieldType::I64, Value::Number(n)) => n.as_i64().is_some(),
    (FieldType::F64, Value::Number(_)) => true,
    (FieldType::Bool, Value::Bool(_)) => true,
    (FieldType::Json, _) => true,
    (FieldType::ArrayString, Value::Array(arr)) => arr.iter().all(|v| v.is_string()),
    (FieldType::ArrayI64, Value::Array(arr)) => arr.iter().all(|v| v.as_i64().is_some()),
    (FieldType::ArrayF64, Value::Array(arr)) => arr.iter().all(|v| v.is_number()),
    _ => false,
  };
  if ok || (field.optional && val.is_null()) {
    Ok(())
  } else {
    Err(Error::from_reason(format!(
      "field '{}' expected {}, got {}",
      field.name,
      type_label(&field.type_),
      val,
    )))
  }
}

/// Validate and optionally rebuild. Returns original Value when no changes needed.
fn validate_record(data: Value, schema: &CompiledSchema) -> Result<Value> {
  match data {
    Value::Object(map) => {
      // First pass: validate all existing fields
      for (key, val) in &map {
        if let Some(&idx) = schema.indices.get(key) {
          validate_value(val, &schema.fields[idx])?;
        }
      }
      // Check missing fields
      let mut needs_null_insert = false;
      for field in &schema.fields {
        if !map.contains_key(&field.name) {
          if field.optional {
            needs_null_insert = true;
          } else {
            return Err(Error::from_reason(format!(
              "missing required field '{}'",
              field.name
            )));
          }
        }
      }
      if needs_null_insert {
        // Rebuild: keep valid fields, add nulls for missing optionals
        let mut out = serde_json::Map::with_capacity(schema.fields.len());
        for field in &schema.fields {
          match map.get(&field.name) {
            Some(v) => {
              out.insert(field.name.clone(), v.clone());
            }
            None => {
              out.insert(field.name.clone(), Value::Null);
            }
          }
        }
        Ok(Value::Object(out))
      } else {
        // Fast path: all fields present and valid, return original map
        Ok(Value::Object(map))
      }
    }
    other => Err(Error::from_reason(format!("expected object, got {}", other))),
  }
}

#[napi]
pub struct DynamicSchema {
  pub(crate) schemas: FxHashMap<String, CompiledSchema>,
}

#[napi]
impl DynamicSchema {
  #[napi(constructor)]
  pub fn new() -> Self {
    DynamicSchema { schemas: FxHashMap::default() }
  }

  #[napi]
  pub fn register(&mut self, schema_name: String, fields: Vec<SchemaField>) -> Result<()> {
    if fields.len() > 64 {
      return Err(Error::from_reason(format!(
        "schema '{}' has {} fields; max 64 supported",
        schema_name,
        fields.len()
      )));
    }
    let mut compiled = Vec::with_capacity(fields.len());
    let mut indices: FxHashMap<String, usize> =
      FxHashMap::with_capacity_and_hasher(fields.len(), Default::default());
    for (i, f) in fields.iter().enumerate() {
      if indices.contains_key(&f.name) {
        return Err(Error::from_reason(format!(
          "duplicate field '{}' in schema '{}'",
          f.name, schema_name
        )));
      }
      indices.insert(f.name.clone(), i);
      compiled.push(CompiledField {
        name: f.name.clone(),
        type_: f.type_.clone(),
        optional: f.optional.unwrap_or(false),
      });
    }
    self.schemas.insert(schema_name, CompiledSchema { fields: compiled, indices });
    Ok(())
  }

  /// Uses streaming parser — validates during JSON tokenization.
  #[napi]
  pub fn parse(&self, schema_name: String, buffer: Buffer) -> Result<Vec<Value>> {
    let schema = self
      .schemas
      .get(&schema_name)
      .ok_or_else(|| Error::from_reason(format!("schema '{schema_name}' not found")))?;
    crate::json_stream::stream_parse_array(&buffer, schema)
      .map_err(|e| Error::from_reason(e))
  }

  /// Same as parse() but from JSON string. Uses streaming parser.
  #[napi]
  pub fn parse_string(&self, schema_name: String, input: String) -> Result<Vec<Value>> {
    let schema = self
      .schemas
      .get(&schema_name)
      .ok_or_else(|| Error::from_reason(format!("schema '{schema_name}' not found")))?;
    crate::json_stream::stream_parse_array(input.as_bytes(), schema)
      .map_err(|e| Error::from_reason(e))
  }

  /// Parse single record and validate against schema. Uses streaming parser.
  #[napi]
  pub fn parse_one(&self, schema_name: String, buffer: Buffer) -> Result<Value> {
    let schema = self
      .schemas
      .get(&schema_name)
      .ok_or_else(|| Error::from_reason(format!("schema '{schema_name}' not found")))?;
    crate::json_stream::stream_parse_one(&buffer, schema)
      .map_err(|e| Error::from_reason(e))
  }

  /// Validate a JS Object by accessing properties directly via napi — no Value intermediate.
  /// Returns the object (no conversion overhead).
  /// Missing optional fields remain absent (not injected as null).
  #[napi]
  pub fn validate_object<'a>(&self, schema_name: String, obj: Object<'a>) -> Result<Object<'a>> {
    let schema = self
      .schemas
      .get(&schema_name)
      .ok_or_else(|| Error::from_reason(format!("schema '{schema_name}' not found")))?;
    for field in &schema.fields {
      let has = obj.has_named_property(&field.name)?;
      if !has {
        if !field.optional {
          return Err(Error::from_reason(format!("missing required field '{}'", field.name)));
        }
        continue;
      }
      match &field.type_ {
        FieldType::String => {
          obj.get_named_property::<String>(&field.name)?;
        }
        FieldType::I64 => {
          obj.get_named_property::<i64>(&field.name)?;
        }
        FieldType::F64 => {
          obj.get_named_property::<f64>(&field.name)?;
        }
        FieldType::Bool => {
          obj.get_named_property::<bool>(&field.name)?;
        }
        FieldType::ArrayString => {
          let arr = obj.get_named_property::<Object>(&field.name)?;
          if !arr.is_array()? {
            return Err(Error::from_reason(format!("field '{}' expected array", field.name)));
          }
          let len = arr.get_array_length()?;
          for i in 0..len {
            arr.get_element::<String>(i)?;
          }
        }
        FieldType::ArrayI64 => {
          let arr = obj.get_named_property::<Object>(&field.name)?;
          if !arr.is_array()? {
            return Err(Error::from_reason(format!("field '{}' expected array", field.name)));
          }
          let len = arr.get_array_length()?;
          for i in 0..len {
            arr.get_element::<i64>(i)?;
          }
        }
        FieldType::ArrayF64 => {
          let arr = obj.get_named_property::<Object>(&field.name)?;
          if !arr.is_array()? {
            return Err(Error::from_reason(format!("field '{}' expected array", field.name)));
          }
          let len = arr.get_array_length()?;
          for i in 0..len {
            arr.get_element::<f64>(i)?;
          }
        }
        _ => {
          obj.get_named_property::<Unknown>(&field.name)?;
        }
      }
    }
    Ok(obj)
  }

  /// Convert a validated Object to a Vec<Option<serde_json::Value>>
  /// following the schema field order, suitable for database insertion.
  #[napi]
  pub fn to_row_values(&self, schema_name: String, obj: Object) -> Result<Vec<Option<Value>>> {
    let schema = self
      .schemas
      .get(&schema_name)
      .ok_or_else(|| Error::from_reason(format!("schema '{schema_name}' not found")))?;

    let mut row = Vec::with_capacity(schema.fields.len());
    for field in &schema.fields {
      let has = obj.has_named_property(&field.name)?;
      if !has {
        row.push(None);
        continue;
      }
      let val: Unknown = obj.get_named_property(&field.name)?;
      match val.get_type()? {
        napi::ValueType::Null | napi::ValueType::Undefined => row.push(None),
        _ => {
          let json_val = crate::database::jv_helper(&val)?;
          row.push(Some(json_val));
        }
      }
    }
    Ok(row)
  }

  /// Get schema for internal use
  pub fn get_schema(&self, schema_name: &str) -> Option<&CompiledSchema> {
    self.schemas.get(schema_name)
  }

  /// Validate a pre-parsed serde_json::Value. Fast path: returns original Value when valid.
  #[napi]
  pub fn validate(&self, schema_name: String, value: Value) -> Result<Value> {
    let schema = self
      .schemas
      .get(&schema_name)
      .ok_or_else(|| Error::from_reason(format!("schema '{schema_name}' not found")))?;
    match value {
      Value::Array(arr) => {
        let out: Result<Vec<Value>> =
          arr.into_iter().map(|r| validate_record(r, schema)).collect();
        Ok(Value::Array(out?))
      }
      other => validate_record(other, schema),
    }
  }
}

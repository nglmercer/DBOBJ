#![allow(unstable_features)]
#![feature(let_chains)]

use napi::bindgen_prelude::*;

mod database;
mod types;

pub use database::Database;
pub use types::{ColumnDefinition, TableMetadata};

#[napi]
pub fn get_column_types() -> serde_json::Value {
    serde_json::json!({
        "Integer": "integer",
        "Float": "float",
        "String": "string",
        "Boolean": "boolean",
        "Blob": "blob"
    })
}

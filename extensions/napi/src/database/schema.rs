use napi_derive::napi;

#[napi]
pub struct Schema {
    pub(crate) db: std::sync::Arc<dbobj::Database>,
}

#[napi]
impl Schema {
    fn get_info(&self, table_name: &str) -> napi::Result<dbobj::core::TableInfo> {
        self.db
            .table_info(table_name)
            .ok_or_else(|| napi::Error::from_reason(format!("Table '{}' not found", table_name)))
    }

    /// Returns a description of schema violations in the given row values.
    /// Returns an empty array if the row is valid.
    #[napi]
    pub fn validate_row(
        &self,
        table_name: String,
        values: Vec<serde_json::Value>,
    ) -> napi::Result<Vec<String>> {
        let info = self.get_info(&table_name)?;
        let mut errors = Vec::new();

        if values.len() != info.columns.len() {
            errors.push(format!(
                "expected {} values, got {}",
                info.columns.len(),
                values.len()
            ));
            return Ok(errors);
        }

        for (i, col) in info.columns.iter().enumerate() {
            let val = &values[i];
            let ok = match (&col.data_type, val) {
                (_, serde_json::Value::Null) => col.nullable,
                (dbobj::DataType::Integer, serde_json::Value::Number(n)) => n.is_i64(),
                (dbobj::DataType::Float, serde_json::Value::Number(_)) => true,
                (dbobj::DataType::String, serde_json::Value::String(_)) => true,
                (dbobj::DataType::Boolean, serde_json::Value::Bool(_)) => true,
                (dbobj::DataType::Blob, serde_json::Value::Array(_)) => true,
                _ => false,
            };
            if !ok {
                errors.push(format!(
                    "column '{}': expected {:?}, got {:?}",
                    col.name, col.data_type, val
                ));
            }
        }
        Ok(errors)
    }

    #[napi]
    pub fn get_column_names(&self, table_name: String) -> napi::Result<Vec<String>> {
        let info = self
            .db
            .table_info(&table_name)
            .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
        Ok(info
            .columns
            .into_iter()
            .map(|c| c.name.to_string())
            .collect())
    }

    #[napi]
    pub fn get_column_type(
        &self,
        table_name: String,
        column_name: String,
    ) -> napi::Result<crate::types::DataType> {
        let info = self
            .db
            .table_info(&table_name)
            .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
        let col = info
            .columns
            .iter()
            .find(|c| c.name.as_str() == column_name)
            .ok_or_else(|| {
                napi::Error::from_reason(format!(
                    "Column '{}' not found in table '{}'",
                    column_name, table_name
                ))
            })?;
        Ok(match col.data_type {
            dbobj::DataType::Integer => crate::types::DataType::Integer,
            dbobj::DataType::Float => crate::types::DataType::Float,
            dbobj::DataType::String => crate::types::DataType::String,
            dbobj::DataType::Boolean => crate::types::DataType::Boolean,
            dbobj::DataType::Blob => crate::types::DataType::Blob,
        })
    }

    #[napi]
    pub fn has_column(&self, table_name: String, column_name: String) -> napi::Result<bool> {
        let info = self
            .db
            .table_info(&table_name)
            .ok_or_else(|| napi::Error::from_reason(format!("Table {} not found", table_name)))?;
        Ok(info.columns.iter().any(|c| c.name.as_str() == column_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbobj::core::{ColumnDefinition, DataType, Database, Schema as CoreSchema};

    fn setup_db() -> (std::sync::Arc<Database>, Schema) {
        let db = std::sync::Arc::new(Database::new("test_db".to_string()));
        let schema = CoreSchema {
            columns: vec![
                ColumnDefinition {
                    name: "id".into(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
                ColumnDefinition {
                    name: "name".into(),
                    data_type: DataType::String,
                    nullable: false,
                },
                ColumnDefinition {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: true,
                },
                ColumnDefinition {
                    name: "active".into(),
                    data_type: DataType::Boolean,
                    nullable: true,
                },
            ],
        };
        db.create_table("users".to_string(), schema);
        // Insert a row so the table exists with data
        let mut row_data = dbobj::core::RowData::default();
        row_data.insert("id".into(), dbobj::core::Value::Integer(1));
        row_data.insert("name".into(), dbobj::core::Value::String("alice".into()));
        row_data.insert("age".into(), dbobj::core::Value::Integer(30));
        row_data.insert("active".into(), dbobj::core::Value::Boolean(true));
        db.insert_row("users", row_data, None).unwrap();

        let s = Schema { db: db.clone() };
        (db, s)
    }

    #[test]
    fn test_validate_row_valid() {
        let (_, schema) = setup_db();
        let values = vec![
            serde_json::Value::Number(serde_json::Number::from(1)),
            serde_json::Value::String("bob".to_string()),
            serde_json::Value::Number(serde_json::Number::from(25)),
            serde_json::Value::Bool(true),
        ];
        let result = schema.validate_row("users".to_string(), values).unwrap();
        assert!(result.is_empty(), "Expected no errors, got: {:?}", result);
    }

    #[test]
    fn test_validate_row_wrong_count() {
        let (_, schema) = setup_db();
        let values = vec![
            serde_json::Value::Number(serde_json::Number::from(1)),
            serde_json::Value::String("bob".to_string()),
        ];
        let result = schema.validate_row("users".to_string(), values).unwrap();
        assert!(!result.is_empty());
        assert!(result[0].contains("expected 4 values"));
    }

    #[test]
    fn test_validate_row_nullable_accepts_null() {
        let (_, schema) = setup_db();
        let values = vec![
            serde_json::Value::Number(serde_json::Number::from(1)),
            serde_json::Value::String("bob".to_string()),
            serde_json::Value::Null, // age is nullable
            serde_json::Value::Null, // active is nullable
        ];
        let result = schema.validate_row("users".to_string(), values).unwrap();
        assert!(result.is_empty(), "Expected no errors, got: {:?}", result);
    }

    #[test]
    fn test_validate_row_type_mismatch() {
        let (_, schema) = setup_db();
        // id column expects Integer, we pass String
        let values = vec![
            serde_json::Value::String("not-a-number".to_string()),
            serde_json::Value::String("bob".to_string()),
            serde_json::Value::Null,
            serde_json::Value::Null,
        ];
        let result = schema.validate_row("users".to_string(), values).unwrap();
        assert!(!result.is_empty());
        assert!(result[0].contains("id"));
        assert!(result[0].contains("Integer"));
    }

    #[test]
    fn test_validate_row_table_not_found() {
        let (_, schema) = setup_db();
        let values = vec![serde_json::Value::Null];
        let result = schema.validate_row("nonexistent".to_string(), values);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_get_column_names() {
        let (_, schema) = setup_db();
        let names = schema.get_column_names("users".to_string()).unwrap();
        assert_eq!(names, vec!["id", "name", "age", "active"]);
    }

    #[test]
    fn test_get_column_type() {
        let (_, schema) = setup_db();
        let t = schema
            .get_column_type("users".to_string(), "id".to_string())
            .unwrap();
        assert!(matches!(t, crate::types::DataType::Integer));
        let t = schema
            .get_column_type("users".to_string(), "name".to_string())
            .unwrap();
        assert!(matches!(t, crate::types::DataType::String));
        let t = schema
            .get_column_type("users".to_string(), "active".to_string())
            .unwrap();
        assert!(matches!(t, crate::types::DataType::Boolean));
    }

    #[test]
    fn test_get_column_type_not_found() {
        let (_, schema) = setup_db();
        let result = schema.get_column_type("users".to_string(), "nonexistent".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_has_column() {
        let (_, schema) = setup_db();
        assert!(schema
            .has_column("users".to_string(), "name".to_string())
            .unwrap());
        assert!(!schema
            .has_column("users".to_string(), "nonexistent".to_string())
            .unwrap());
    }

    #[test]
    fn test_get_info_table_not_found() {
        let (_, schema) = setup_db();
        let result = schema.get_column_names("nonexistent".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}

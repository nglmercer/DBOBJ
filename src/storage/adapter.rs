use super::StorageError;
use crate::core::Database;

/// Adapter trait for Database serialization
pub trait SerializerAdapter {
    fn serialize(&self, db: &Database) -> Result<Vec<u8>, StorageError>;
    fn deserialize(&self, bytes: &[u8]) -> Result<Database, StorageError>;
}

/// Bitcode implementation
pub struct BitcodeAdapter;

impl SerializerAdapter for BitcodeAdapter {
    fn serialize(&self, db: &Database) -> Result<Vec<u8>, StorageError> {
        bitcode::serialize(db).map_err(|e| StorageError::Serialization(e.to_string()))
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<Database, StorageError> {
        bitcode::deserialize(bytes).map_err(|e| StorageError::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ColumnDefinition, DataType, RowData, Schema, Value};

    fn create_test_db() -> Database {
        let db = Database::new("TestDB".to_string());
        let schema = Schema {
            columns: vec![ColumnDefinition {
                name: "col1".into(),
                data_type: DataType::String,
                nullable: false,
            }],
        };
        db.create_table("test_table".to_string(), schema);
        let mut row = RowData::default();
        row.insert("col1".into(), Value::from("hello"));
        db.insert_row("test_table", row, None).unwrap();
        db
    }

    #[test]
    fn test_bitcode_adapter() {
        let db = create_test_db();
        let adapter = BitcodeAdapter;

        let bytes = adapter
            .serialize(&db)
            .expect("Failed to serialize with Bitcode");
        assert!(!bytes.is_empty());

        let loaded_db = adapter
            .deserialize(&bytes)
            .expect("Failed to deserialize with Bitcode");
        assert_eq!(db.name, loaded_db.name);
        assert_eq!(
            loaded_db.get_table("test_table").unwrap().read().ids.len(),
            1
        );
    }
}

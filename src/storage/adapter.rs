use crate::core::Database;
use super::StorageError;

/// Adapter trait for Database serialization
pub trait SerializerAdapter {
    fn serialize(&self, db: &Database) -> Result<Vec<u8>, StorageError>;
    fn deserialize(&self, bytes: &[u8]) -> Result<Database, StorageError>;
}

/// Bincode implementation
pub struct BincodeAdapter;

impl SerializerAdapter for BincodeAdapter {
    fn serialize(&self, db: &Database) -> Result<Vec<u8>, StorageError> {
        let config = bincode::config::standard();
        bincode::serde::encode_to_vec(db, config)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<Database, StorageError> {
        let config = bincode::config::standard();
        let (db, _): (Database, usize) = bincode::serde::decode_from_slice(bytes, config)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(db)
    }
}

/// Highly optimized Bincode implementation
pub struct FastBincodeAdapter;

impl SerializerAdapter for FastBincodeAdapter {
    fn serialize(&self, db: &Database) -> Result<Vec<u8>, StorageError> {
        let config = bincode::config::standard().with_fixed_int_encoding();
        bincode::serde::encode_to_vec(db, config)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<Database, StorageError> {
        let config = bincode::config::standard().with_fixed_int_encoding();
        let (db, _): (Database, usize) = bincode::serde::decode_from_slice(bytes, config)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        Ok(db)
    }
}

/// Postcard implementation
pub struct PostcardAdapter;

impl SerializerAdapter for PostcardAdapter {
    fn serialize(&self, db: &Database) -> Result<Vec<u8>, StorageError> {
        postcard::to_stdvec(db)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<Database, StorageError> {
        postcard::from_bytes(bytes)
            .map_err(|e| StorageError::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Schema, ColumnDefinition, DataType, Value, RowData};

    fn create_test_db() -> Database {
        let mut db = Database::new("TestDB".to_string());
        let schema = Schema {
            columns: vec![
                ColumnDefinition {
                    name: "col1".into(),
                    data_type: DataType::String,
                    nullable: false,
                }
            ],
        };
        db.create_table("test_table".to_string(), schema);
        let mut row = RowData::default();
        row.insert("col1".into(), Value::from("hello"));
        db.insert_row("test_table", row, None).unwrap();
        db
    }

    #[test]
    fn test_bincode_adapter() {
        let db = create_test_db();
        let adapter = BincodeAdapter;
        
        let bytes = adapter.serialize(&db).expect("Failed to serialize with Bincode");
        assert!(!bytes.is_empty());
        
        let loaded_db = adapter.deserialize(&bytes).expect("Failed to deserialize with Bincode");
        assert_eq!(db.name, loaded_db.name);
        assert_eq!(loaded_db.get_table("test_table").unwrap().rows.len(), 1);
    }

    #[test]
    fn test_postcard_adapter() {
        let db = create_test_db();
        let adapter = PostcardAdapter;
        
        let bytes = adapter.serialize(&db).expect("Failed to serialize with Postcard");
        assert!(!bytes.is_empty());
        
        let loaded_db = adapter.deserialize(&bytes).expect("Failed to deserialize with Postcard");
        assert_eq!(db.name, loaded_db.name);
        assert_eq!(loaded_db.get_table("test_table").unwrap().rows.len(), 1);
    }

    #[test]
    fn test_fast_bincode_adapter() {
        let db = create_test_db();
        let adapter = FastBincodeAdapter;
        
        let bytes = adapter.serialize(&db).expect("Failed to serialize with FastBincode");
        assert!(!bytes.is_empty());
        
        let loaded_db = adapter.deserialize(&bytes).expect("Failed to deserialize with FastBincode");
        assert_eq!(db.name, loaded_db.name);
        assert_eq!(loaded_db.get_table("test_table").unwrap().rows.len(), 1);
    }
}

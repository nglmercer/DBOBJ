use super::database::Database;
use super::table::Table;
use super::value::Value;
use super::{ColumnDefinition, DataType, Id, Schema};

use arrow::array::*;
use arrow::datatypes::{DataType as ArrowDataType, Field, Schema as ArrowSchema};
use arrow::ipc::reader::FileReader;
use arrow::ipc::writer::FileWriter;
use arrow::record_batch::RecordBatch;
use std::sync::Arc;

fn db_to_arrow_type(dt: &DataType) -> ArrowDataType {
    match dt {
        DataType::Integer => ArrowDataType::Int64,
        DataType::Float => ArrowDataType::Float64,
        DataType::String => ArrowDataType::Utf8,
        DataType::Boolean => ArrowDataType::Boolean,
        DataType::Blob => ArrowDataType::Binary,
    }
}

fn arrow_to_db_type(dt: &ArrowDataType) -> Option<DataType> {
    match dt {
        ArrowDataType::Int8
        | ArrowDataType::Int16
        | ArrowDataType::Int32
        | ArrowDataType::Int64
        | ArrowDataType::UInt8
        | ArrowDataType::UInt16
        | ArrowDataType::UInt32
        | ArrowDataType::UInt64 => Some(DataType::Integer),
        ArrowDataType::Float16 | ArrowDataType::Float32 | ArrowDataType::Float64 => {
            Some(DataType::Float)
        }
        ArrowDataType::Utf8 | ArrowDataType::LargeUtf8 => Some(DataType::String),
        ArrowDataType::Boolean => Some(DataType::Boolean),
        ArrowDataType::Binary | ArrowDataType::LargeBinary | ArrowDataType::FixedSizeBinary(_) => {
            Some(DataType::Blob)
        }
        _ => None,
    }
}

impl Table {
    pub fn to_arrow_schema(&self) -> Arc<ArrowSchema> {
        let fields: Vec<Field> = self
            .schema
            .columns
            .iter()
            .map(|col| {
                let arrow_type = db_to_arrow_type(&col.data_type);
                Field::new(col.name.as_str(), arrow_type, col.nullable)
            })
            .collect();
        Arc::new(ArrowSchema::new(fields))
    }

    pub fn to_record_batch(&self) -> Result<RecordBatch, String> {
        let arrow_schema = self.to_arrow_schema();
        let num_rows = self.ids.len();
        let num_cols = self.num_columns;

        let mut arrays: Vec<Arc<dyn Array>> = Vec::with_capacity(num_cols);

        for (col_idx, col) in self.schema.columns.iter().enumerate() {
            let array: Arc<dyn Array> = match col.data_type {
                DataType::Integer => {
                    let mut builder = Int64Builder::with_capacity(num_rows);
                    for i in 0..num_rows {
                        match &self.data[i * num_cols + col_idx] {
                            Value::Null => builder.append_null(),
                            Value::Integer(v) => builder.append_value(*v),
                            _ => builder.append_null(),
                        }
                    }
                    Arc::new(builder.finish())
                }
                DataType::Float => {
                    let mut builder = Float64Builder::with_capacity(num_rows);
                    for i in 0..num_rows {
                        match &self.data[i * num_cols + col_idx] {
                            Value::Null => builder.append_null(),
                            Value::Float(v) => builder.append_value(*v),
                            _ => builder.append_null(),
                        }
                    }
                    Arc::new(builder.finish())
                }
                DataType::String => {
                    let mut builder = StringBuilder::with_capacity(num_rows, num_rows * 32);
                    for i in 0..num_rows {
                        let val = &self.data[i * num_cols + col_idx];
                        match val {
                            Value::Null => builder.append_null(),
                            Value::String(s) => builder.append_value(s.as_str()),
                            Value::InternedString(id) => {
                                if let Some(s) = self.string_pool.resolve(*id) {
                                    builder.append_value(s.as_str());
                                } else {
                                    builder.append_null();
                                }
                            }
                            _ => builder.append_null(),
                        }
                    }
                    Arc::new(builder.finish())
                }
                DataType::Boolean => {
                    let mut builder = BooleanBuilder::with_capacity(num_rows);
                    for i in 0..num_rows {
                        match &self.data[i * num_cols + col_idx] {
                            Value::Null => builder.append_null(),
                            Value::Boolean(v) => builder.append_value(*v),
                            _ => builder.append_null(),
                        }
                    }
                    Arc::new(builder.finish())
                }
                DataType::Blob => {
                    let mut builder = BinaryBuilder::with_capacity(num_rows, num_rows * 128);
                    for i in 0..num_rows {
                        match &self.data[i * num_cols + col_idx] {
                            Value::Null => builder.append_null(),
                            Value::Blob(v) => builder.append_value(v),
                            _ => builder.append_null(),
                        }
                    }
                    Arc::new(builder.finish())
                }
            };
            arrays.push(array);
        }

        RecordBatch::try_new(arrow_schema, arrays)
            .map_err(|e| format!("Failed to create RecordBatch: {}", e))
    }

    pub fn from_record_batch(name: String, batch: &RecordBatch) -> Result<Self, String> {
        let arrow_schema = batch.schema();
        let num_rows = batch.num_rows();
        let num_cols = batch.num_columns();

        let mut schema_columns = Vec::with_capacity(num_cols);
        for i in 0..num_cols {
            let field = arrow_schema.field(i);
            let db_type = arrow_to_db_type(field.data_type())
                .ok_or_else(|| format!("Unsupported Arrow type: {:?}", field.data_type()))?;
            schema_columns.push(ColumnDefinition {
                name: field.name().into(),
                data_type: db_type,
                nullable: field.is_nullable(),
            });
        }

        let db_schema = Schema {
            columns: schema_columns,
        };
        let mut table = Table::new(name, db_schema);
        table.data.reserve(num_rows * num_cols);
        table.ids.reserve(num_rows);
        table.versions.reserve(num_rows);

        for row_idx in 0..num_rows {
            for col_idx in 0..num_cols {
                let arr = batch.column(col_idx);
                let val = match arrow_schema.field(col_idx).data_type() {
                    ArrowDataType::Int64 => {
                        let a = arr.as_any().downcast_ref::<Int64Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Integer(a.value(row_idx))
                        }
                    }
                    ArrowDataType::Int32 => {
                        let a = arr.as_any().downcast_ref::<Int32Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Integer(a.value(row_idx) as i64)
                        }
                    }
                    ArrowDataType::Int16 => {
                        let a = arr.as_any().downcast_ref::<Int16Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Integer(a.value(row_idx) as i64)
                        }
                    }
                    ArrowDataType::Int8 => {
                        let a = arr.as_any().downcast_ref::<Int8Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Integer(a.value(row_idx) as i64)
                        }
                    }
                    ArrowDataType::UInt64 => {
                        let a = arr.as_any().downcast_ref::<UInt64Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Integer(a.value(row_idx) as i64)
                        }
                    }
                    ArrowDataType::UInt32 => {
                        let a = arr.as_any().downcast_ref::<UInt32Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Integer(a.value(row_idx) as i64)
                        }
                    }
                    ArrowDataType::UInt16 => {
                        let a = arr.as_any().downcast_ref::<UInt16Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Integer(a.value(row_idx) as i64)
                        }
                    }
                    ArrowDataType::UInt8 => {
                        let a = arr.as_any().downcast_ref::<UInt8Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Integer(a.value(row_idx) as i64)
                        }
                    }
                    ArrowDataType::Float64 => {
                        let a = arr.as_any().downcast_ref::<Float64Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Float(a.value(row_idx))
                        }
                    }
                    ArrowDataType::Float32 => {
                        let a = arr.as_any().downcast_ref::<Float32Array>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Float(a.value(row_idx) as f64)
                        }
                    }
                    ArrowDataType::Utf8 | ArrowDataType::LargeUtf8 => {
                        let a = arr.as_any().downcast_ref::<StringArray>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::String(compact_str::CompactString::from(a.value(row_idx)))
                        }
                    }
                    ArrowDataType::Boolean => {
                        let a = arr.as_any().downcast_ref::<BooleanArray>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Boolean(a.value(row_idx))
                        }
                    }
                    ArrowDataType::Binary | ArrowDataType::LargeBinary => {
                        let a = arr.as_any().downcast_ref::<BinaryArray>().unwrap();
                        if a.is_null(row_idx) {
                            Value::Null
                        } else {
                            Value::Blob(a.value(row_idx).to_vec())
                        }
                    }
                    t => {
                        return Err(format!("Unsupported Arrow type: {:?}", t));
                    }
                };
                table.data.push(val);
            }

            let id = Id::Integer(table.next_int_id);
            table.next_int_id += 1;
            table.ids.push(id);
            table.versions.push(1);
        }

        Ok(table)
    }

    pub fn to_arrow_ipc(&self) -> Result<Vec<u8>, String> {
        let batch = self.to_record_batch()?;
        let schema = self.to_arrow_schema();

        let mut buffer = Vec::new();
        {
            let mut writer = FileWriter::try_new(&mut buffer, &schema)
                .map_err(|e| format!("Failed to create Arrow writer: {}", e))?;
            writer
                .write(&batch)
                .map_err(|e| format!("Failed to write RecordBatch: {}", e))?;
            writer
                .finish()
                .map_err(|e| format!("Failed to finish Arrow writer: {}", e))?;
        }
        Ok(buffer)
    }

    pub fn from_arrow_ipc(name: String, bytes: &[u8]) -> Result<Self, String> {
        let cursor = std::io::Cursor::new(bytes);
        let reader = FileReader::try_new(cursor, None)
            .map_err(|e| format!("Failed to read Arrow IPC: {}", e))?;

        let arrow_schema = reader.schema();

        let mut schema_columns = Vec::with_capacity(arrow_schema.fields().len());
        for field in arrow_schema.fields() {
            let db_type = arrow_to_db_type(field.data_type())
                .ok_or_else(|| format!("Unsupported Arrow type: {:?}", field.data_type()))?;
            schema_columns.push(ColumnDefinition {
                name: field.name().into(),
                data_type: db_type,
                nullable: field.is_nullable(),
            });
        }
        let db_schema = Schema {
            columns: schema_columns,
        };
        let mut table = Table::new(name, db_schema);

        for maybe_batch in reader {
            let batch = maybe_batch.map_err(|e| format!("Failed to read RecordBatch: {}", e))?;
            let num_rows = batch.num_rows();
            let num_cols = batch.num_columns();

            table.data.reserve(num_rows * num_cols);
            table.ids.reserve(num_rows);

            for row_idx in 0..num_rows {
                for col_idx in 0..num_cols {
                    let arr = batch.column(col_idx);
                    let val = match arrow_schema.field(col_idx).data_type() {
                        ArrowDataType::Int64 => {
                            let a = arr.as_any().downcast_ref::<Int64Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Integer(a.value(row_idx))
                            }
                        }
                        ArrowDataType::Int32 => {
                            let a = arr.as_any().downcast_ref::<Int32Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Integer(a.value(row_idx) as i64)
                            }
                        }
                        ArrowDataType::Int16 => {
                            let a = arr.as_any().downcast_ref::<Int16Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Integer(a.value(row_idx) as i64)
                            }
                        }
                        ArrowDataType::Int8 => {
                            let a = arr.as_any().downcast_ref::<Int8Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Integer(a.value(row_idx) as i64)
                            }
                        }
                        ArrowDataType::UInt64 => {
                            let a = arr.as_any().downcast_ref::<UInt64Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Integer(a.value(row_idx) as i64)
                            }
                        }
                        ArrowDataType::UInt32 => {
                            let a = arr.as_any().downcast_ref::<UInt32Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Integer(a.value(row_idx) as i64)
                            }
                        }
                        ArrowDataType::UInt16 => {
                            let a = arr.as_any().downcast_ref::<UInt16Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Integer(a.value(row_idx) as i64)
                            }
                        }
                        ArrowDataType::UInt8 => {
                            let a = arr.as_any().downcast_ref::<UInt8Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Integer(a.value(row_idx) as i64)
                            }
                        }
                        ArrowDataType::Float64 => {
                            let a = arr.as_any().downcast_ref::<Float64Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Float(a.value(row_idx))
                            }
                        }
                        ArrowDataType::Float32 => {
                            let a = arr.as_any().downcast_ref::<Float32Array>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Float(a.value(row_idx) as f64)
                            }
                        }
                        ArrowDataType::Utf8 | ArrowDataType::LargeUtf8 => {
                            let a = arr.as_any().downcast_ref::<StringArray>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::String(compact_str::CompactString::from(a.value(row_idx)))
                            }
                        }
                        ArrowDataType::Boolean => {
                            let a = arr.as_any().downcast_ref::<BooleanArray>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Boolean(a.value(row_idx))
                            }
                        }
                        ArrowDataType::Binary | ArrowDataType::LargeBinary => {
                            let a = arr.as_any().downcast_ref::<BinaryArray>().unwrap();
                            if a.is_null(row_idx) {
                                Value::Null
                            } else {
                                Value::Blob(a.value(row_idx).to_vec())
                            }
                        }
                        t => {
                            return Err(format!("Unsupported Arrow type: {:?}", t));
                        }
                    };
                    table.data.push(val);
                }

                let id = Id::Integer(table.next_int_id);
                table.next_int_id += 1;
                table.ids.push(id);
                table.versions.push(1);
            }
        }

        Ok(table)
    }
}

impl Database {
    pub fn export_table_to_arrow_ipc(&self, table_name: &str) -> Result<Vec<u8>, String> {
        let tables_guard = self.tables.read();
        let table_lock = tables_guard
            .get(table_name)
            .ok_or_else(|| format!("Table '{}' not found", table_name))?;
        let table = table_lock.read();
        table.to_arrow_ipc()
    }

    pub fn import_table_from_arrow_ipc(
        &self,
        table_name: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        let table = Table::from_arrow_ipc(table_name.to_string(), bytes)?;
        let mut tables_guard = self.tables.write();
        tables_guard.insert(
            table_name.to_string(),
            Arc::new(parking_lot::RwLock::new(table)),
        );
        Ok(())
    }
}

use crate::protocol::{
    ColumnDef, ComparisonOp, ExprData, Request, Response, SerializedRow,
};
use async_trait::async_trait;
use dbobj::Database;
use std::sync::Arc;

/// Abstract database backend
#[async_trait]
pub trait Backend: Send + Sync {
    async fn execute(&self, req: Request) -> Response;
}

/// Backend implementation wrapping the DBOBJ database
pub struct DbobjBackend {
    db: Arc<Database>,
}

impl DbobjBackend {
    pub fn new(db: Arc<Database>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }
}

fn convert_data_type(dt: &str) -> dbobj::DataType {
    match dt {
        "Integer" => dbobj::DataType::Integer,
        "Float" => dbobj::DataType::Float,
        "String" | "Text" => dbobj::DataType::String,
        "Boolean" | "Bool" => dbobj::DataType::Boolean,
        "Blob" | "Bytes" => dbobj::DataType::Blob,
        _ => dbobj::DataType::String,
    }
}

#[async_trait]
impl Backend for DbobjBackend {
    async fn execute(&self, req: Request) -> Response {
        match req {
            Request::CreateTable { name, columns } => {
                let schema = dbobj::Schema {
                    columns: columns
                        .into_iter()
                        .map(|c| dbobj::ColumnDefinition {
                            name: c.name.into(),
                            data_type: convert_data_type(&c.data_type),
                            nullable: c.nullable,
                        })
                        .collect(),
                };
                self.db.create_table(name, schema);
                Response::Ok(1)
            }

            Request::DropTable { name } => match self.db.drop_table(&name) {
                Ok(_) => Response::Ok(1),
                Err(e) => Response::Error(e.to_string()),
            },

            Request::ListTables => {
                let tables = self.db.list_tables();
                Response::TableList(tables)
            }

            Request::TableInfo { name } => match self.db.table_info(&name) {
                Some(info) => Response::TableInfo {
                    name: info.name,
                    columns: info
                        .columns
                        .into_iter()
                        .map(|c| ColumnDef {
                            name: c.name.to_string(),
                            data_type: format!("{:?}", c.data_type),
                            nullable: c.nullable,
                        })
                        .collect(),
                    row_count: info.row_count,
                },
                None => Response::Error(format!("Table '{}' not found", name)),
            },

            Request::Insert {
                table,
                data,
                custom_id,
            } => match self.db.insert_row(&table, data, custom_id) {
                Ok(id) => Response::Id(id),
                Err(e) => Response::Error(e.to_string()),
            },

            Request::InsertValues { table, values } => {
                match self.db.insert_values(&table, values) {
                    Ok(id) => Response::Id(id),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::InsertBatch { table, batch } => {
                match self.db.insert_batch(&table, batch) {
                    Ok(ids) => Response::Ids(ids),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::InsertBatchValues { table, batch } => {
                match self.db.insert_batch_values(&table, batch) {
                    Ok(ids) => Response::Ids(ids),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::InsertOrReplace {
                table,
                values,
                unique_column,
            } => match self.db.insert_or_replace(&table, values, &unique_column) {
                Ok(id) => Response::Id(id),
                Err(e) => Response::Error(e.to_string()),
            },

            Request::UpdateRow { table, id, data } => {
                match self.db.update_row(&table, &id, data) {
                    Ok(_) => Response::Ok(1),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::UpdateValues {
                table,
                id,
                values,
            } => match self.db.update_values(&table, &id, values) {
                Ok(_) => Response::Ok(1),
                Err(e) => Response::Error(e.to_string()),
            },

            Request::UpdateByIndices {
                table,
                id,
                updates,
            } => match self.db.update_row_by_indices(&table, &id, &updates) {
                Ok(_) => Response::Ok(1),
                Err(e) => Response::Error(e.to_string()),
            },

            Request::DeleteRow { table, id } => match self.db.delete_row(&table, &id) {
                Ok(_) => Response::Ok(1),
                Err(e) => Response::Error(e.to_string()),
            },

            Request::DeleteBatch { table, ids } => match self.db.delete_batch(&table, &ids) {
                Ok(n) => Response::Ok(n as u64),
                Err(e) => Response::Error(e.to_string()),
            },

            Request::Query {
                table,
                column_name,
                value,
            } => {
                match self.db.find(&table, &column_name, value) {
                    Ok(rows) => {
                        let serialized: Vec<SerializedRow> = rows
                            .into_iter()
                            .map(|r| SerializedRow {
                                id: r.id,
                                data: r.data.to_vec(),
                            })
                            .collect();
                        Response::Rows(serialized)
                    }
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::QueryPredicate {
                table,
                column_idx,
                operator,
                value,
            } => {
                // Build an expression and use query_expr
                let col_name = {
                    let tbl = match self.db.get_table(&table) {
                        Some(t) => t,
                        None => {
                            return Response::Error(format!("Table '{}' not found", table))
                        }
                    };
                    let tbl_guard = tbl.read();
                    let column_map = &tbl_guard.column_map;
                    match column_map.iter().find(|(_, idx)| **idx == column_idx) {
                        Some((name, _)) => name.clone(),
                        None => return Response::Error(format!("Column index {} not found", column_idx)),
                    }
                };

                let expr = match operator {
                    ComparisonOp::Eq => ExprData::Binary {
                        left: Box::new(ExprData::Column(col_name)),
                        op: ComparisonOp::Eq,
                        right: Box::new(ExprData::Literal(value)),
                    },
                    ComparisonOp::Neq => ExprData::Binary {
                        left: Box::new(ExprData::Column(col_name)),
                        op: ComparisonOp::Neq,
                        right: Box::new(ExprData::Literal(value)),
                    },
                    ComparisonOp::Gt => ExprData::Binary {
                        left: Box::new(ExprData::Column(col_name)),
                        op: ComparisonOp::Gt,
                        right: Box::new(ExprData::Literal(value)),
                    },
                    ComparisonOp::Gte => ExprData::Binary {
                        left: Box::new(ExprData::Column(col_name)),
                        op: ComparisonOp::Gte,
                        right: Box::new(ExprData::Literal(value)),
                    },
                    ComparisonOp::Lt => ExprData::Binary {
                        left: Box::new(ExprData::Column(col_name)),
                        op: ComparisonOp::Lt,
                        right: Box::new(ExprData::Literal(value)),
                    },
                    ComparisonOp::Lte => ExprData::Binary {
                        left: Box::new(ExprData::Column(col_name)),
                        op: ComparisonOp::Lte,
                        right: Box::new(ExprData::Literal(value)),
                    },
                };

                self.handle_expr_query(&table, expr)
            }

            Request::QueryExpr { table, expr } => self.handle_expr_query(&table, expr),

            Request::CreateIndex { table, column } => {
                match self.db.create_index(&table, &column) {
                    Ok(_) => Response::Ok(1),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::CreateUniqueIndex { table, column } => {
                match self.db.create_unique_index(&table, &column) {
                    Ok(_) => Response::Ok(1),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::HashJoin {
                table1,
                col1,
                table2,
                col2,
            } => match self.db.hash_join(&table1, &col1, &table2, &col2) {
                Ok(rows) => {
                    let joined: Vec<(SerializedRow, SerializedRow)> = rows
                        .into_iter()
                        .map(|(a, b)| {
                            (
                                SerializedRow {
                                    id: a.id,
                                    data: a.data.to_vec(),
                                },
                                SerializedRow {
                                    id: b.id,
                                    data: b.data.to_vec(),
                                },
                            )
                        })
                        .collect();
                    Response::JoinedRows(joined)
                }
                Err(e) => Response::Error(e.to_string()),
            },

            Request::BeginTransaction => {
                // Transactions are client-side in DBOBJ's model;
                // we can wrap the snapshot-based approach
                // For now, just acknowledge
                Response::Ok(1)
            }

            Request::CommitTransaction => Response::Ok(1),
            Request::RollbackTransaction => Response::Ok(1),

            Request::Save => {
                // Save to default path using the Storage adapter
                let storage =
                    dbobj::storage::Storage::new("dbobj_server.db", dbobj::storage::BitcodeAdapter);
                match storage.save(&self.db) {
                    Ok(_) => Response::Ok(1),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::Load { path } => {
                let storage =
                    dbobj::storage::Storage::new(&path, dbobj::storage::BitcodeAdapter);
                match storage.load() {
                    Ok(_loaded) => Response::Ok(1),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::Ping => Response::Pong,
        }
    }
}

impl DbobjBackend {
    fn handle_expr_query(&self, table: &str, expr: ExprData) -> Response {
        // Convert ExprData to dbobj::Expr
        let db_expr = match convert_expr(expr) {
            Ok(e) => e,
            Err(msg) => return Response::Error(msg),
        };

        match self.db.query_expr(table, db_expr) {
            Ok(rows) => {
                let serialized: Vec<SerializedRow> = rows
                    .into_iter()
                    .map(|r| SerializedRow {
                        id: r.id,
                        data: r.data.to_vec(),
                    })
                    .collect();
                Response::Rows(serialized)
            }
            Err(e) => Response::Error(e.to_string()),
        }
    }
}

fn convert_expr(expr: ExprData) -> Result<dbobj::Expr, String> {
    match expr {
        ExprData::Column(name) => Ok(dbobj::Expr::Column(name.into())),
        ExprData::Literal(val) => Ok(dbobj::Expr::Literal(val)),
        ExprData::Binary { left, op, right } => {
            let left_expr = convert_expr(*left)?;
            let right_expr = convert_expr(*right)?;
            let db_op = convert_op(op);
            Ok(dbobj::Expr::Binary(
                Box::new(left_expr),
                db_op,
                Box::new(right_expr),
            ))
        }
        ExprData::And(exprs) => {
            let mut iter = exprs.into_iter();
            let first = iter
                .next()
                .ok_or_else(|| "AND requires at least one expression".to_string())?;
            let mut result = convert_expr(first)?;
            for e in iter {
                let next = convert_expr(e)?;
                result = dbobj::Expr::Binary(
                    Box::new(result),
                    dbobj::Operator::And,
                    Box::new(next),
                );
            }
            Ok(result)
        }
        ExprData::Or(exprs) => {
            let mut iter = exprs.into_iter();
            let first = iter
                .next()
                .ok_or_else(|| "OR requires at least one expression".to_string())?;
            let mut result = convert_expr(first)?;
            for e in iter {
                let next = convert_expr(e)?;
                result = dbobj::Expr::Binary(
                    Box::new(result),
                    dbobj::Operator::Or,
                    Box::new(next),
                );
            }
            Ok(result)
        }
        ExprData::Not(inner) => {
            let inner_expr = convert_expr(*inner)?;
            Ok(dbobj::Expr::Not(Box::new(inner_expr)))
        }
    }
}

fn convert_op(op: ComparisonOp) -> dbobj::Operator {
    match op {
        ComparisonOp::Eq => dbobj::Operator::Eq,
        ComparisonOp::Neq => dbobj::Operator::Neq,
        ComparisonOp::Gt => dbobj::Operator::Gt,
        ComparisonOp::Gte => dbobj::Operator::Gte,
        ComparisonOp::Lt => dbobj::Operator::Lt,
        ComparisonOp::Lte => dbobj::Operator::Lte,
    }
}
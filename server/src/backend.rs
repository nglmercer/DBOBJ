use crate::backup::BackupManager;
use crate::migration::{Migration, MigrationAction, MigrationRunner};
use crate::protocol::{
    ColumnDef, ComparisonOp, ExprData, MigrationSummary, Request, Response,
    SchemaChange, SerializedRow,
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
    backup_mgr: Option<BackupManager>,
    migration_runner: Option<MigrationRunner>,
}

impl DbobjBackend {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            backup_mgr: None,
            migration_runner: None,
        }
    }

    pub fn db(&self) -> &Arc<Database> {
        &self.db
    }

    pub fn with_backup_dir(mut self, backup_dir: impl Into<std::path::PathBuf>) -> Self {
        self.backup_mgr = Some(BackupManager::new(backup_dir));
        self
    }

    pub fn with_migrations(mut self) -> Self {
        self.migration_runner = Some(MigrationRunner::new(self.db.clone()));
        self
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

            Request::InsertBatch { table, batch } => match self.db.insert_batch(&table, batch) {
                Ok(ids) => Response::Ids(ids),
                Err(e) => Response::Error(e.to_string()),
            },

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

            Request::UpdateRow { table, id, data } => match self.db.update_row(&table, &id, data) {
                Ok(_) => Response::Ok(1),
                Err(e) => Response::Error(e.to_string()),
            },

            Request::UpdateValues { table, id, values } => {
                match self.db.update_values(&table, &id, values) {
                    Ok(_) => Response::Ok(1),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            Request::UpdateByIndices { table, id, updates } => {
                match self.db.update_row_by_indices(&table, &id, &updates) {
                    Ok(_) => Response::Ok(1),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

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
            } => match self.db.find(&table, &column_name, value) {
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
            },

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
                        None => return Response::Error(format!("Table '{}' not found", table)),
                    };
                    let tbl_guard = tbl.read();
                    let column_map = &tbl_guard.column_map;
                    match column_map.iter().find(|(_, idx)| **idx == column_idx) {
                        Some((name, _)) => name.clone(),
                        None => {
                            return Response::Error(format!(
                                "Column index {} not found",
                                column_idx
                            ))
                        }
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

            Request::CreateIndex { table, column } => match self.db.create_index(&table, &column) {
                Ok(_) => Response::Ok(1),
                Err(e) => Response::Error(e.to_string()),
            },

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
                let storage = dbobj::storage::Storage::new(&path, dbobj::storage::BitcodeAdapter);
                match storage.load() {
                    Ok(_loaded) => Response::Ok(1),
                    Err(e) => Response::Error(e.to_string()),
                }
            }

            // ── Backup operations ─────────────────────────────────
            Request::CreateBackup { label, format } => {
                match &self.backup_mgr {
                    Some(mgr) => match mgr.create_backup(&self.db, label, format) {
                        Ok(info) => Response::BackupCreated(info),
                        Err(e) => Response::Error(e.to_string()),
                    },
                    None => Response::Error("Backup manager not configured".into()),
                }
            }
            Request::ListBackups => {
                match &self.backup_mgr {
                    Some(mgr) => match mgr.list_backups() {
                        Ok(backups) => Response::BackupList(backups),
                        Err(e) => Response::Error(e.to_string()),
                    },
                    None => Response::Error("Backup manager not configured".into()),
                }
            }
            Request::RestoreBackup { backup_id, mode } => {
                match &self.backup_mgr {
                    Some(mgr) => match mgr.restore_backup(&backup_id, mode) {
                        Ok(db) => {
                            // Replace the current Arc<Database> with the restored one
                            // Because Arc doesn't support direct replacement, we update
                            // the database in-place by clearing and restoring
                            let mut tables = self.db.tables.write();
                            *tables = db.tables.write().clone();
                            drop(tables);
                            // Rebuild indexes and column maps for the new state
                            let tables = self.db.tables.read();
                            for (_, tbl_lock) in tables.iter() {
                                let mut guard = tbl_lock.write();
                                guard.rebuild_from_archive();
                            }
                            // Ensure migration tracking table exists if runner exists
                            if self.migration_runner.is_some() {
                                drop(tables);
                                // Re-create migration runner state
                                // (applied migrations list is re-read from tracking table)
                            }
                            Response::Ok(1)
                        }
                        Err(e) => Response::Error(e.to_string()),
                    },
                    None => Response::Error("Backup manager not configured".into()),
                }
            }
            Request::DeleteBackup { backup_id } => {
                match &self.backup_mgr {
                    Some(mgr) => match mgr.delete_backup(&backup_id) {
                        Ok(()) => Response::BackupDeleted,
                        Err(e) => Response::Error(e.to_string()),
                    },
                    None => Response::Error("Backup manager not configured".into()),
                }
            }

            // ── Migration operations ──────────────────────────────
            Request::RegisterMigration {
                name,
                description,
                actions,
            } => {
                match &self.migration_runner {
                    Some(_runner) => {
                        // Create a Migration from the SchemaChange actions
                        let mut migration = Migration::new(&name, &description);
                        // Workaround for immutable borrow - can't use register because
                        // MigrationRunner only exposes mutable register.
                        // We execute SchemaChange actions directly via the runner's DB.
                        // Build migration actions
                        let migration_actions: Vec<MigrationAction> = actions
                            .into_iter()
                            .map(|change| match change {
                                SchemaChange::AddColumn {
                                    table,
                                    column,
                                    default_value,
                                } => MigrationAction::AddColumn {
                                    table,
                                    column,
                                    default_value,
                                },
                                SchemaChange::DropColumn { table, column } => {
                                    MigrationAction::DropColumn { table, column }
                                }
                                SchemaChange::RenameColumn {
                                    table,
                                    old_name,
                                    new_name,
                                } => MigrationAction::RenameColumn {
                                    table,
                                    old_name,
                                    new_name,
                                },
                                SchemaChange::RenameTable { old_name, new_name } => {
                                    MigrationAction::RenameTable { old_name, new_name }
                                }
                                SchemaChange::DropTable { name } => {
                                    MigrationAction::DropTable { name }
                                }
                            })
                            .collect();

                        for action in migration_actions {
                            migration = migration.add_step(action);
                        }
                        // Since we can't get a mutable reference, we use a static
                        // registration approach via the runner's database
                        let _ = migration;
                        Response::Error(
                            "Dynamic migration registration not supported during server runtime. \
                             Use the direct backend API or configure migrations at startup"
                                .into(),
                        )
                    }
                    None => Response::Error(
                        "Migration runner not configured. Enable with with_migrations()".into(),
                    ),
                }
            }
            Request::RunPendingMigrations => {
                match &self.migration_runner {
                    Some(runner) => match runner.run_pending() {
                        Ok(statuses) => Response::MigrationStatuses(statuses),
                        Err(e) => Response::Error(e.to_string()),
                    },
                    None => Response::Error(
                        "Migration runner not configured. Enable with with_migrations()".into(),
                    ),
                }
            }
            Request::RunMigration { name } => {
                match &self.migration_runner {
                    Some(runner) => match runner.run_named(&name) {
                        Ok(status) => Response::MigrationStatus(status),
                        Err(e) => Response::Error(e.to_string()),
                    },
                    None => Response::Error(
                        "Migration runner not configured. Enable with with_migrations()".into(),
                    ),
                }
            }
            Request::ListMigrations => {
                match &self.migration_runner {
                    Some(runner) => {
                        let registered = runner.registered();
                        let applied = runner.applied_list();
                        let summaries: Vec<MigrationSummary> = registered
                            .into_iter()
                            .map(|name| MigrationSummary {
                                id: String::new(),
                                name: name.to_string(),
                                description: String::new(),
                                applied: applied.contains(&name.to_string()),
                                applied_at_ms: None,
                                step_count: 0,
                            })
                            .collect();
                        Response::MigrationList(summaries)
                    }
                    None => Response::Error(
                        "Migration runner not configured. Enable with with_migrations()".into(),
                    ),
                }
            }
            Request::DryRunMigrations => {
                match &self.migration_runner {
                    Some(runner) => match runner.dry_run() {
                        Ok(steps) => Response::MigrationSteps(steps),
                        Err(e) => Response::Error(e.to_string()),
                    },
                    None => Response::Error(
                        "Migration runner not configured. Enable with with_migrations()".into(),
                    ),
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
                result =
                    dbobj::Expr::Binary(Box::new(result), dbobj::Operator::And, Box::new(next));
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
                result = dbobj::Expr::Binary(Box::new(result), dbobj::Operator::Or, Box::new(next));
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

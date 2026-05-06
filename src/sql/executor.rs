use crate::core::{Database, RowData, ColumnDefinition, Schema};
use crate::sql::parser::SqlParser;
use sqlparser::ast::{Statement, SetExpr, Query, TableFactor, Expr as SqlExpr, TableObject};
use compact_str::CompactString;

pub struct SqlExecutor<'a> {
    db: &'a Database,
}

impl<'a> SqlExecutor<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn execute(&self, sql: &str) -> Result<SqlResult, String> {
        let statements = SqlParser::parse(sql)?;
        let mut last_result = SqlResult::Ok;

        for stmt in statements {
            last_result = self.execute_statement(stmt)?;
        }

        Ok(last_result)
    }

    fn execute_statement(&self, stmt: Statement) -> Result<SqlResult, String> {
        match stmt {
            Statement::CreateTable(create_table) => {
                let mut col_defs = Vec::new();
                for col in create_table.columns {
                    col_defs.push(ColumnDefinition {
                        name: CompactString::from(col.name.value.clone()),
                        data_type: SqlParser::map_data_type(&col.data_type)?,
                        nullable: true, // Simplified
                    });
                }
                let schema = Schema { columns: col_defs };
                self.db.create_table(create_table.name.to_string(), schema);
                Ok(SqlResult::Ok)
            }
            Statement::Insert(insert) => {
                let table_name_str = match insert.table {
                    TableObject::TableName(name) => name.to_string(),
                    _ => return Err("Unsupported table object in INSERT".to_string()),
                };

                if let Some(source) = insert.source {
                    if let SetExpr::Values(values) = *source.body {
                        let mut rows = Vec::new();
                        for row_values in values.rows {
                            let mut row_data = RowData::default();
                            let table_lock = if insert.columns.is_empty() {
                                Some(self.db.get_table(&table_name_str).ok_or_else(|| format!("Table {} not found", table_name_str))?)
                            } else {
                                None
                            };
                            let table_guard = table_lock.as_ref().map(|l| l.read());

                            for (i, val_expr) in row_values.into_iter().enumerate() {
                                if let SqlExpr::Value(val_with_span) = val_expr {
                                    let value = SqlParser::map_value(&val_with_span.value)?;
                                    if i < insert.columns.len() {
                                        row_data.insert(CompactString::from(insert.columns[i].value.clone()), value);
                                    } else if let Some(table) = &table_guard {
                                        // Handle positional insert if columns are not specified
                                        if i < table.schema.columns.len() {
                                            row_data.insert(table.schema.columns[i].name.clone(), value);
                                        }
                                    }
                                }
                            }
                            rows.push(row_data);
                        }
                        for row in rows {
                            self.db.insert_row(&table_name_str, row, None)
                                .map_err(|e| e.to_string())?;
                        }
                    }
                }
                Ok(SqlResult::Ok)
            }
            Statement::Query(query) => {
                self.execute_query(*query)
            }
            _ => Err(format!("Unsupported statement: {:?}", stmt)),
        }
    }

    fn execute_query(&self, query: Query) -> Result<SqlResult, String> {
        if let SetExpr::Select(select) = *query.body {
            let select = *select;
            if let Some(TableFactor::Table { name, .. }) = select.from.first().map(|f| &f.relation) {
                let table_name = name.to_string();

                let rows = if let Some(selection) = select.selection {
                    let expr = SqlParser::map_expr(&selection)?;
                    self.db.query_expr(&table_name, expr).map_err(|e| e.to_string())?
                } else {
                    let table_lock = self.db.get_table(&table_name)
                        .ok_or_else(|| format!("Table {} not found", table_name))?;
                    let table = table_lock.read();
                    (0..table.ids.len()).map(|i| table.get_row_by_index(i)).collect()
                };

                let table_lock = self.db.get_table(&table_name).unwrap();
                let table = table_lock.read();
                let results = rows.into_iter().map(|r| r.to_map(&table)).collect();
                Ok(SqlResult::Rows(results))
            } else {
                Err("Unsupported FROM clause".to_string())
            }
        } else {
            Err("Unsupported query body".to_string())
        }
    }
}

#[derive(Debug)]
pub enum SqlResult {
    Ok,
    Rows(Vec<RowData>),
}

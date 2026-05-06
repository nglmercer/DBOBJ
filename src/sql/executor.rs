use std::cell::RefCell;

use crate::core::{ColumnDefinition, Database, RowData, Schema, Value};
use crate::sql::parser::SqlParser;
use compact_str::CompactString;
use sqlparser::ast::{
    AlterTableOperation, AssignmentTarget, Expr as SqlExpr, FromTable, Join, JoinConstraint,
    JoinOperator, Query, SetExpr, Statement, TableFactor, TableObject,
};

const MAX_CACHE_SIZE: usize = 512;

/// Type for SQL statement cache: maps SQL text to parsed AST.
pub type StatementCache = crate::core::FastHashMap<String, Vec<Statement>>;

pub struct SqlExecutor<'a> {
    db: &'a Database,
    stmt_cache: RefCell<StatementCache>,
}

impl<'a> SqlExecutor<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            stmt_cache: RefCell::new(crate::core::FastHashMap::default()),
        }
    }

    pub fn execute(&self, sql: &str) -> Result<SqlResult, String> {
        let statements = self.get_or_parse(sql)?;
        let mut last_result = SqlResult::Ok;
        for stmt in statements {
            last_result = self.execute_statement(stmt)?;
        }
        Ok(last_result)
    }

    /// Execute SQL using an external statement cache (avoids re-parsing across executor instances).
    pub fn execute_with_cache(
        &self,
        sql: &str,
        ext_cache: &RefCell<StatementCache>,
    ) -> Result<SqlResult, String> {
        let statements = {
            let cache = ext_cache.borrow();
            if let Some(cached) = cache.get(sql) {
                cached.clone()
            } else {
                drop(cache);
                let parsed = SqlParser::parse(sql)?;
                let mut cache = ext_cache.borrow_mut();
                if cache.len() >= MAX_CACHE_SIZE {
                    cache.clear();
                }
                cache.insert(sql.to_string(), parsed.clone());
                parsed
            }
        };
        let mut last_result = SqlResult::Ok;
        for stmt in statements {
            last_result = self.execute_statement(stmt)?;
        }
        Ok(last_result)
    }

    fn get_or_parse(&self, sql: &str) -> Result<Vec<Statement>, String> {
        let cache = self.stmt_cache.borrow();
        if let Some(cached) = cache.get(sql) {
            return Ok(cached.clone());
        }
        drop(cache);
        let parsed = SqlParser::parse(sql)?;
        let mut cache = self.stmt_cache.borrow_mut();
        if cache.len() >= MAX_CACHE_SIZE {
            cache.clear();
        }
        cache.insert(sql.to_string(), parsed.clone());
        Ok(parsed)
    }

    fn populate_id_map_once(&self, table_name: &str) -> Result<(), String> {
        let table_lock = self
            .db
            .get_table(table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;
        let mut table = table_lock.write();
        if table.is_sequential_ids {
            table.is_sequential_ids = false;
            let ids = table.ids.clone();
            for (i, id) in ids.into_iter().enumerate() {
                table.id_map.insert(id, i);
            }
        }
        Ok(())
    }

    /// Fast path: if the WHERE clause is `id = literal` AND there is no real
    /// column named "id" in the schema, use the internal row ID directly.
    /// This avoids a full table scan for `SELECT/UPDATE/DELETE ... WHERE id = ?`.
    /// Returns None if the table has a real "id" column (the WHERE refers to column data).
    fn try_extract_id_filter(
        selection: &SqlExpr,
        has_id_column: bool,
    ) -> Option<crate::core::Id> {
        // Only use the fast path when "id" is the internal row ID, not a data column
        if has_id_column {
            return None;
        }
        if let SqlExpr::BinaryOp { left, op, right } = selection
            && matches!(op, sqlparser::ast::BinaryOperator::Eq) {
                let val_expr = match (left.as_ref(), right.as_ref()) {
                    (SqlExpr::Identifier(id), val) if id.value == "id" => val,
                    (val, SqlExpr::Identifier(id)) if id.value == "id" => val,
                    (SqlExpr::CompoundIdentifier(parts), val)
                        if parts.last().map(|p| p.value.as_str()) == Some("id") =>
                    {
                        val
                    }
                    (val, SqlExpr::CompoundIdentifier(parts))
                        if parts.last().map(|p| p.value.as_str()) == Some("id") =>
                    {
                        val
                    }
                    _ => return None,
                };
                if let SqlExpr::Value(val) = val_expr
                    && let Ok(Value::Integer(n)) = SqlParser::map_value(&val.value) {
                        return Some(crate::core::Id::Integer(n as u64));
                    }
            }
        None
    }

    fn execute_statement(&self, stmt: Statement) -> Result<SqlResult, String> {
        match stmt {
            Statement::CreateTable(create_table) => {
                let mut col_defs = Vec::new();
                for col in create_table.columns {
                    col_defs.push(ColumnDefinition {
                        name: CompactString::from(col.name.value.clone()),
                        data_type: SqlParser::map_data_type(&col.data_type)?,
                        nullable: true,
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

                if let Some(source) = insert.source
                    && let SetExpr::Values(values) = *source.body
                {
                    let mut rows = Vec::new();
                    for row_values in values.rows {
                        let mut row_data = RowData::default();
                        let table_lock = if insert.columns.is_empty() {
                            Some(
                                self.db
                                    .get_table(&table_name_str)
                                    .ok_or_else(|| format!("Table {} not found", table_name_str))?,
                            )
                        } else {
                            None
                        };
                        let table_guard = table_lock.as_ref().map(|l| l.read());

                        for (i, val_expr) in row_values.into_iter().enumerate() {
                            if let SqlExpr::Value(val_with_span) = val_expr {
                                let value = SqlParser::map_value(&val_with_span.value)?;
                                if i < insert.columns.len() {
                                    row_data.insert(
                                        CompactString::from(insert.columns[i].value.clone()),
                                        value,
                                    );
                                } else if let Some(table) = &table_guard
                                    && i < table.schema.columns.len() {
                                        row_data
                                            .insert(table.schema.columns[i].name.clone(), value);
                                    }
                            }
                        }
                        rows.push(row_data);
                    }
                    for row in rows {
                        self.db
                            .insert_row(&table_name_str, row, None)
                            .map_err(|e| e.to_string())?;
                    }
                }
                Ok(SqlResult::Ok)
            }
            Statement::Update(update) => {
                let table_name = match &update.table.relation {
                    TableFactor::Table { name, .. } => name.to_string(),
                    _ => return Err("Unsupported table relation in UPDATE".to_string()),
                };

                self.populate_id_map_once(&table_name)?;

                let (ids, rows_data): (Vec<_>, Vec<_>) = {
                    let table_lock = self
                        .db
                        .get_table(&table_name)
                        .ok_or_else(|| format!("Table {} not found", table_name))?;
                    let table_ref = table_lock.read();
                    let has_id_column = table_ref.column_map.contains_key("id");
                    let rows_to_update = if let Some(direct_id) = update
                        .selection
                        .as_ref()
                        .and_then(|s| Self::try_extract_id_filter(s, has_id_column))
                    {
                        if let Some(row) = table_ref.get(&direct_id) {
                            vec![row]
                        } else {
                            Vec::new()
                        }
                    } else if let Some(selection) = &update.selection {
                        let expr = SqlParser::map_expr(selection)?;
                        let mut mapping = table_ref.column_map.clone();
                        for (col, idx) in &table_ref.column_map {
                            mapping.insert(format!("{}.{}", table_name, col), *idx);
                        }
                        if !mapping.contains_key("id") {
                            mapping.insert("id".to_string(), usize::MAX);
                        }
                        table_ref.select(|r| expr.is_true(r, &mapping, &table_ref))
                    } else {
                        (0..table_ref.ids.len())
                            .map(|i| table_ref.get_row_by_index(i))
                            .collect()
                    };
                    rows_to_update
                        .into_iter()
                        .map(|r| (r.id.clone(), r.to_map(&table_ref)))
                        .unzip()
                };

                for (id, mut row_data) in ids.into_iter().zip(rows_data) {
                    for assignment in &update.assignments {
                        let col_name = match &assignment.target {
                            AssignmentTarget::ColumnName(name) => name.to_string(),
                            _ => return Err("Unsupported assignment target".to_string()),
                        };
                        let val = SqlParser::map_expr(&assignment.value)?;
                        if let crate::core::Expr::Literal(v) = val {
                            row_data.insert(CompactString::from(col_name), v);
                        }
                    }
                    self.db
                        .update_row(&table_name, &id, row_data)
                        .map_err(|e| e.to_string())?;
                }
                Ok(SqlResult::Ok)
            }
            Statement::Delete(delete) => {
                let table_with_joins = match delete.from {
                    FromTable::WithFromKeyword(v) => v,
                    FromTable::WithoutKeyword(v) => v,
                };
                let table_with_join = table_with_joins.first().ok_or("Empty FROM in DELETE")?;
                let table_name = match &table_with_join.relation {
                    TableFactor::Table { name, .. } => name.to_string(),
                    _ => return Err("Unsupported table relation in DELETE".to_string()),
                };

                self.populate_id_map_once(&table_name)?;

                let ids: Vec<_> = {
                    let table_lock = self
                        .db
                        .get_table(&table_name)
                        .ok_or_else(|| format!("Table {} not found", table_name))?;
                    let table_ref = table_lock.read();
                    let has_id_column = table_ref.column_map.contains_key("id");
                    let rows_to_delete = if let Some(direct_id) = delete
                        .selection
                        .as_ref()
                        .and_then(|s| Self::try_extract_id_filter(s, has_id_column))
                    {
                        if let Some(row) = table_ref.get(&direct_id) {
                            vec![row]
                        } else {
                            Vec::new()
                        }
                    } else if let Some(selection) = &delete.selection {
                        let expr = SqlParser::map_expr(selection)?;
                        let mut mapping = table_ref.column_map.clone();
                        for (col, idx) in &table_ref.column_map {
                            mapping.insert(format!("{}.{}", table_name, col), *idx);
                        }
                        if !mapping.contains_key("id") {
                            mapping.insert("id".to_string(), usize::MAX);
                        }
                        table_ref.select(|r| expr.is_true(r, &mapping, &table_ref))
                    } else {
                        (0..table_ref.ids.len())
                            .map(|i| table_ref.get_row_by_index(i))
                            .collect()
                    };
                    rows_to_delete.into_iter().map(|r| r.id.clone()).collect()
                };

                for id in ids {
                    self.db
                        .delete_row(&table_name, &id)
                        .map_err(|e| e.to_string())?;
                }
                Ok(SqlResult::Ok)
            }
            Statement::AlterTable(alter_table) => {
                let table_name = alter_table.name.to_string();
                for op in alter_table.operations {
                    if let AlterTableOperation::AddColumn { column_def, .. } = op {
                        let table_lock = self
                            .db
                            .get_table(&table_name)
                            .ok_or_else(|| format!("Table {} not found", table_name))?;
                        let mut table = table_lock.write();

                        let col_name = CompactString::from(column_def.name.value.clone());
                        let data_type = SqlParser::map_data_type(&column_def.data_type)?;

                        let mut new_columns = table.schema.columns.clone();
                        new_columns.push(ColumnDefinition {
                            name: col_name.clone(),
                            data_type,
                            nullable: true,
                        });

                        let old_num_columns = table.num_columns;
                        table.schema.columns = new_columns;
                        table.num_columns += 1;
                        table
                            .column_map
                            .insert(col_name.to_string(), old_num_columns);

                        let num_rows = table.ids.len();
                        let mut new_data = Vec::with_capacity(num_rows * table.num_columns);
                        for i in 0..num_rows {
                            let old_start = i * old_num_columns;
                            let old_end = old_start + old_num_columns;
                            for j in old_start..old_end {
                                new_data.push(table.data[j].clone());
                            }
                            new_data.push(Value::Null);
                        }
                        table.data = new_data;
                        if table.is_sequential_ids {
                            table.is_sequential_ids = false;
                            let ids = table.ids.clone();
                            for (i, id) in ids.into_iter().enumerate() {
                                table.id_map.insert(id, i);
                            }
                        }
                    }
                }
                Ok(SqlResult::Ok)
            }
            Statement::Query(query) => self.execute_query(*query),
            _ => Err(format!("Unsupported statement: {:?}", stmt)),
        }
    }

    fn execute_query(&self, query: Query) -> Result<SqlResult, String> {
        if let SetExpr::Select(select) = *query.body {
            let select = *select;
            if let Some(table_with_joins) = select.from.first() {
                let table_name = match &table_with_joins.relation {
                    TableFactor::Table { name, .. } => name.to_string(),
                    _ => return Err("Unsupported FROM clause".to_string()),
                };

                if let Some(join) = table_with_joins.joins.first() {
                    return self.execute_join(&table_name, join);
                }

                let table_lock = self
                    .db
                    .get_table(&table_name)
                    .ok_or_else(|| format!("Table {} not found", table_name))?;
                let table_ref = table_lock.read();

                let has_id_column = table_ref.column_map.contains_key("id");
                let fast_id = select
                    .selection
                    .as_ref()
                    .and_then(|s| Self::try_extract_id_filter(s, has_id_column));

                // If fast path would be used, ensure id_map is populated first
                if fast_id.is_some() {
                    drop(table_ref);
                    drop(table_lock);
                    self.populate_id_map_once(&table_name)?;
                }

                let table_lock = self
                    .db
                    .get_table(&table_name)
                    .ok_or_else(|| format!("Table {} not found", table_name))?;
                let table_ref = table_lock.read();

                let rows = if let Some(direct_id) = fast_id {
                    if let Some(row) = table_ref.get(&direct_id) {
                        vec![row]
                    } else {
                        Vec::new()
                    }
                } else if let Some(selection) = &select.selection {
                    let expr = SqlParser::map_expr(selection)?;
                    let mut mapping = table_ref.column_map.clone();
                    for (col, idx) in &table_ref.column_map {
                        mapping.insert(format!("{}.{}", table_name, col), *idx);
                    }
                    if !mapping.contains_key("id") {
                        mapping.insert("id".to_string(), usize::MAX);
                    }
                    table_ref.select(|r| expr.is_true(r, &mapping, &table_ref))
                } else {
                    (0..table_ref.ids.len())
                        .map(|i| table_ref.get_row_by_index(i))
                        .collect()
                };

                let results = rows.into_iter().map(|r| r.to_map(&table_ref)).collect();
                Ok(SqlResult::Rows(results))
            } else {
                Err("Missing FROM clause".to_string())
            }
        } else {
            Err("Unsupported query body".to_string())
        }
    }

    fn execute_join(&self, table1_name: &str, join: &Join) -> Result<SqlResult, String> {
        let table2_name = match &join.relation {
            TableFactor::Table { name, .. } => name.to_string(),
            _ => return Err("Unsupported join relation".to_string()),
        };

        match &join.join_operator {
            JoinOperator::Inner(constraint) => {
                if let JoinConstraint::On(expr) = constraint {
                    if let SqlExpr::BinaryOp { left, op, right } = expr
                        && matches!(op, sqlparser::ast::BinaryOperator::Eq)
                        && let (
                            SqlExpr::CompoundIdentifier(left_parts),
                            SqlExpr::CompoundIdentifier(right_parts),
                        ) = (left.as_ref(), right.as_ref())
                    {
                        let (t1_col, t2_col) = if left_parts[0].value == table1_name {
                            (left_parts[1].value.as_str(), right_parts[1].value.as_str())
                        } else {
                            (right_parts[1].value.as_str(), left_parts[1].value.as_str())
                        };

                        let joined_rows = self
                            .db
                            .hash_join(table1_name, t1_col, &table2_name, t2_col)
                            .map_err(|e| e.to_string())?;

                        let t1_lock = self.db.get_table(table1_name).unwrap();
                        let t1 = t1_lock.read();
                        let t2_lock = self.db.get_table(&table2_name).unwrap();
                        let t2 = t2_lock.read();

                        let mut results = Vec::new();
                        for (r1, r2) in joined_rows {
                            let mut m1 = r1.to_map(&t1);
                            let m2 = r2.to_map(&t2);
                            let mut combined = RowData::default();
                            for (k, v) in m1.drain() {
                                combined.insert(
                                    CompactString::from(format!("{}.{}", table1_name, k)),
                                    v,
                                );
                            }
                            for (k, v) in m2 {
                                combined.insert(
                                    CompactString::from(format!("{}.{}", table2_name, k)),
                                    v,
                                );
                            }
                            results.push(combined);
                        }
                        return Ok(SqlResult::Rows(results));
                    }
                    Err("Only simple equality joins on columns are supported".to_string())
                } else {
                    Err("Unsupported join constraint".to_string())
                }
            }
            _ => Err("Unsupported join operator".to_string()),
        }
    }
}

#[derive(Debug)]
pub enum SqlResult {
    Ok,
    Rows(Vec<RowData>),
}

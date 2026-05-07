use std::cell::RefCell;

use crate::core::{ColumnDefinition, Database, RowData, Schema, Value};
use crate::sql::parser::SqlParser;
use compact_str::CompactString;
use sqlparser::ast::{
    AlterTableOperation, AssignmentTarget, Expr as SqlExpr, FromTable, Join, JoinConstraint,
    JoinOperator, Query, SetExpr, Statement, TableFactor, TableObject, Value as SqlValue,
    ValueWithSpan,
};

const MAX_CACHE_SIZE: usize = 512;

/// Type for SQL statement cache: maps SQL text to parsed AST.
pub type StatementCache = crate::core::FastHashMap<String, Vec<Statement>>;

/// A pre-parsed SQL statement with parameter placeholders.
/// Call `execute(&[Value::from(1), Value::from("alice")])` to run with bound parameters.
#[derive(Clone)]
pub struct PreparedStatement {
    statements: Vec<Statement>,
    param_count: usize,
}

impl PreparedStatement {
    /// Count the number of `?` placeholders in the SQL AST.
    fn count_params(stmts: &[Statement]) -> usize {
        let mut count = 0;
        for stmt in stmts {
            count_stmt_placeholders(stmt, &mut count);
        }
        count
    }
}

fn count_stmt_placeholders(stmt: &Statement, count: &mut usize) {
    match stmt {
        Statement::Query(q) => count_query_placeholders(q, count),
        Statement::Insert(insert) => {
            if let Some(source) = &insert.source
                && let SetExpr::Values(values) = &*source.body
            {
                for row in &values.rows {
                    for val in row {
                        count_expr_placeholders(val, count);
                    }
                }
            }
        }
        Statement::Update(update) => {
            for assign in &update.assignments {
                count_expr_placeholders(&assign.value, count);
            }
            if let Some(sel) = &update.selection {
                count_expr_placeholders(sel, count);
            }
        }
        Statement::Delete(delete) => {
            if let Some(sel) = &delete.selection {
                count_expr_placeholders(sel, count);
            }
        }
        _ => {}
    }
}

fn count_query_placeholders(q: &Query, count: &mut usize) {
    if let SetExpr::Select(select) = &*q.body {
        if let Some(sel) = &select.selection {
            count_expr_placeholders(sel, count);
        }
    } else if let SetExpr::Values(values) = &*q.body {
        for row in &values.rows {
            for val in row {
                count_expr_placeholders(val, count);
            }
        }
    }
}

fn count_expr_placeholders(expr: &SqlExpr, count: &mut usize) {
    match expr {
        SqlExpr::Value(v) => {
            if matches!(&v.value, SqlValue::Placeholder(_)) {
                *count += 1;
            }
        }
        SqlExpr::BinaryOp { left, op: _, right } => {
            count_expr_placeholders(left, count);
            count_expr_placeholders(right, count);
        }
        SqlExpr::UnaryOp { op: _, expr } => count_expr_placeholders(expr, count),
        SqlExpr::Nested(expr) => count_expr_placeholders(expr, count),
        SqlExpr::IsNull(expr) | SqlExpr::IsNotNull(expr) => count_expr_placeholders(expr, count),
        SqlExpr::Between { expr, low, high, .. } => {
            count_expr_placeholders(expr, count);
            count_expr_placeholders(low, count);
            count_expr_placeholders(high, count);
        }
        SqlExpr::InList { expr, list, .. } => {
            count_expr_placeholders(expr, count);
            for item in list {
                count_expr_placeholders(item, count);
            }
        }
        SqlExpr::Function(_) => {}
        SqlExpr::Cast { expr, .. } => count_expr_placeholders(expr, count),
        SqlExpr::Subquery(q) => count_query_placeholders(q, count),
        _ => {}
    }
}

/// Substitute parameter values into a cloned Statement AST.
fn substitute_statement(stmt: &Statement, params: &[Value], idx: &mut usize) -> Statement {
    match stmt {
        Statement::Query(q) => Statement::Query(Box::new(substitute_query(q, params, idx))),
        Statement::Insert(insert) => {
            let mut insert = insert.clone();
            if let Some(source) = &insert.source
                && let SetExpr::Values(values) = source.body.as_ref()
            {
                let mut new_values = values.clone();
                for row in &mut new_values.rows {
                    for val in row {
                        *val = substitute_expr(val, params, idx);
                    }
                }
                insert.source = Some(Box::new(Query {
                    body: Box::new(SetExpr::Values(new_values)),
                    ..*source.clone()
                }));
            }
            Statement::Insert(insert)
        }
        Statement::Update(update) => {
            let mut update = update.clone();
            for assign in &mut update.assignments {
                assign.value = substitute_expr(&assign.value, params, idx);
            }
            if let Some(sel) = &update.selection {
                update.selection = Some(substitute_expr(sel, params, idx));
            }
            Statement::Update(update)
        }
        Statement::Delete(delete) => {
            let mut delete = delete.clone();
            if let Some(sel) = &delete.selection {
                delete.selection = Some(substitute_expr(sel, params, idx));
            }
            Statement::Delete(delete)
        }
        _ => stmt.clone(),
    }
}

fn substitute_query(q: &Query, params: &[Value], idx: &mut usize) -> Query {
    let mut q = q.clone();
    match q.body.as_mut() {
        SetExpr::Select(select) => {
            if let Some(sel) = &select.selection {
                select.selection = Some(substitute_expr(sel, params, idx));
            }
            q.body = Box::new(SetExpr::Select(select.clone()));
        }
        SetExpr::Values(values) => {
            let mut new_values = values.clone();
            for row in &mut new_values.rows {
                for val in row {
                    *val = substitute_expr(val, params, idx);
                }
            }
            q.body = Box::new(SetExpr::Values(new_values));
        }
        _ => {}
    }
    q
}

fn substitute_expr(expr: &SqlExpr, params: &[Value], idx: &mut usize) -> SqlExpr {
    match expr {
        SqlExpr::Value(v) => {
            if let SqlValue::Placeholder(_) = &v.value {
                let val = &params[*idx];
                *idx += 1;
                SqlExpr::Value(ValueWithSpan {
                    value: core_value_to_sql_value(val),
                    span: v.span,
                })
            } else {
                SqlExpr::Value(v.clone())
            }
        }
        SqlExpr::BinaryOp { left, op, right } => SqlExpr::BinaryOp {
            left: Box::new(substitute_expr(left, params, idx)),
            op: op.clone(),
            right: Box::new(substitute_expr(right, params, idx)),
        },
        SqlExpr::UnaryOp { op, expr: inner } => SqlExpr::UnaryOp {
            op: op.clone(),
            expr: Box::new(substitute_expr(inner, params, idx)),
        },
        SqlExpr::Nested(inner) => SqlExpr::Nested(Box::new(substitute_expr(inner, params, idx))),
        SqlExpr::IsNull(inner) => SqlExpr::IsNull(Box::new(substitute_expr(inner, params, idx))),
        SqlExpr::IsNotNull(inner) => {
            SqlExpr::IsNotNull(Box::new(substitute_expr(inner, params, idx)))
        }
        SqlExpr::Between {
            expr: e,
            negated,
            low,
            high,
        } => SqlExpr::Between {
            expr: Box::new(substitute_expr(e, params, idx)),
            negated: *negated,
            low: Box::new(substitute_expr(low, params, idx)),
            high: Box::new(substitute_expr(high, params, idx)),
        },
        SqlExpr::InList {
            expr: e,
            list,
            negated,
        } => SqlExpr::InList {
            expr: Box::new(substitute_expr(e, params, idx)),
            list: list
                .iter()
                .map(|item| substitute_expr(item, params, idx))
                .collect(),
            negated: *negated,
        },
        SqlExpr::Function(_) => expr.clone(),
        SqlExpr::Cast {
            expr: e,
            data_type,
            format,
            kind,
            array,
        } => SqlExpr::Cast {
            expr: Box::new(substitute_expr(e, params, idx)),
            data_type: data_type.clone(),
            format: format.clone(),
            kind: kind.clone(),
            array: *array,
        },
        SqlExpr::Subquery(q) => SqlExpr::Subquery(Box::new(substitute_query(q, params, idx))),
        _ => expr.clone(),
    }
}

/// Convert a DBOBJ core Value to a sqlparser Value representation.
fn core_value_to_sql_value(val: &Value) -> SqlValue {
    match val {
        Value::Integer(n) => SqlValue::Number(n.to_string(), false),
        Value::Float(f) => {
            let s = if f.is_nan() {
                "NaN".to_string()
            } else {
                format!("{}", f)
            };
            SqlValue::Number(s, false)
        }
        Value::String(s) => SqlValue::SingleQuotedString(s.to_string()),
        Value::InternedString(_) => SqlValue::SingleQuotedString("".to_string()),
        Value::Boolean(b) => SqlValue::Boolean(*b),
        Value::Null => SqlValue::Null,
        Value::Blob(_) => SqlValue::SingleQuotedString("".to_string()),
    }
}

pub struct SqlExecutor<'a> {
    db: &'a Database,
    stmt_cache: RefCell<StatementCache>,
    /// Tables whose id_map has already been populated from sequential IDs.
    /// Avoids repeated write-lock acquisitions for the no-op populate check.
    populated: RefCell<Vec<String>>,
}

impl<'a> SqlExecutor<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            stmt_cache: RefCell::new(crate::core::FastHashMap::default()),
            populated: RefCell::new(Vec::new()),
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

    /// Parse and cache a SQL statement with `?` placeholders for later execution
    /// with bound parameter values. Returns a `PreparedStatement` that can be
    /// executed repeatedly with different parameters without re-parsing.
    ///
    /// ```ignore
    /// let stmt = executor.prepare("SELECT * FROM users WHERE id = ?")?;
    /// let result = executor.execute_prepared(&stmt, &[Value::from(42)])?;
    /// ```
    pub fn prepare(&self, sql: &str) -> Result<PreparedStatement, String> {
        let statements = {
            let cache = self.stmt_cache.borrow();
            if let Some(cached) = cache.get(sql) {
                cached.clone()
            } else {
                drop(cache);
                let parsed = SqlParser::parse(sql)?;
                let mut cache = self.stmt_cache.borrow_mut();
                if cache.len() >= MAX_CACHE_SIZE {
                    cache.clear();
                }
                cache.insert(sql.to_string(), parsed.clone());
                parsed
            }
        };
        Ok(PreparedStatement {
            param_count: PreparedStatement::count_params(&statements),
            statements,
        })
    }

    /// Execute a prepared statement with bound parameter values.
    /// The number of parameters must match the number of `?` placeholders.
    pub fn execute_prepared(
        &self,
        stmt: &PreparedStatement,
        params: &[Value],
    ) -> Result<SqlResult, String> {
        if params.len() != stmt.param_count {
            return Err(format!(
                "Expected {} parameters, got {}",
                stmt.param_count,
                params.len()
            ));
        }
        let mut param_idx = 0;
        let mut last_result = SqlResult::Ok;
        for statement in &stmt.statements {
            let resolved = substitute_statement(statement, params, &mut param_idx);
            last_result = self.execute_statement(resolved)?;
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
        {
            let populated = self.populated.borrow();
            if populated.contains(&table_name.to_string()) {
                return Ok(());
            }
        }
        let table_lock = self
            .db
            .get_table(table_name)
            .ok_or_else(|| format!("Table {} not found", table_name))?;
        let mut table = table_lock.write();
        if table.is_sequential_ids {
            table.is_sequential_ids = false;
            let id_pairs: Vec<_> = table.ids.iter().enumerate().map(|(i, id)| (i, id.clone())).collect();
            for (i, id) in id_pairs {
                table.id_map.insert(id, i);
            }
        }
        self.populated.borrow_mut().push(table_name.to_string());
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
                    // Batch insert when multiple rows
                    if rows.len() > 1 {
                        self.db
                            .insert_batch(&table_name_str, rows)
                            .map_err(|e| e.to_string())?;
                    } else if let Some(row) = rows.into_iter().next() {
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
                        let cap = table_ref.column_map.len() * 2 + 1;
                        let mut mapping = crate::core::FastHashMap::with_capacity_and_hasher(cap, Default::default());
                        for (col, idx) in &table_ref.column_map {
                            mapping.insert(col.clone(), *idx);
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
                        let cap = table_ref.column_map.len() * 2 + 1;
                        let mut mapping = crate::core::FastHashMap::with_capacity_and_hasher(cap, Default::default());
                        for (col, idx) in &table_ref.column_map {
                            mapping.insert(col.clone(), *idx);
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

                let already_populated = self.populated.borrow().contains(&table_name);

                if fast_id.is_some() && !already_populated {
                    drop(table_ref);
                    drop(table_lock);
                    self.populate_id_map_once(&table_name)?;
                    let table_lock = self
                        .db
                        .get_table(&table_name)
                        .ok_or_else(|| format!("Table {} not found", table_name))?;
                    let table_ref = table_lock.read();
                    let results: Vec<RowData> = if let Some(direct_id) = fast_id
                        && let Some(row) = table_ref.get(&direct_id)
                    {
                        vec![row.to_map(&table_ref)]
                    } else {
                        Vec::new()
                    };
                    return Ok(SqlResult::Rows(results));
                }

                let rows = if let Some(direct_id) = fast_id
                    && let Some(row) = table_ref.get(&direct_id)
                {
                    vec![row]
                } else if let Some(selection) = &select.selection {
                    if let Some((col_name, value)) = Self::try_extract_eq_literal(selection) {
                        Self::lookup_indexed_or_scan(&table_ref, &col_name, &value)
                    } else {
                        let expr = SqlParser::map_expr(selection)?;
                        let cap = table_ref.column_map.len() * 2 + 1;
                        let mut mapping = crate::core::FastHashMap::with_capacity_and_hasher(cap, Default::default());
                        for (col, idx) in &table_ref.column_map {
                            mapping.insert(col.clone(), *idx);
                            mapping.insert(format!("{}.{}", table_name, col), *idx);
                        }
                        if !mapping.contains_key("id") {
                            mapping.insert("id".to_string(), usize::MAX);
                        }
                        table_ref.select(|r| expr.is_true(r, &mapping, &table_ref))
                    }
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

    /// Try to extract `column = literal` from a simple WHERE expression. Returns (col_name, value).
    fn try_extract_eq_literal(selection: &SqlExpr) -> Option<(String, Value)> {
        if let SqlExpr::BinaryOp { left, op, right } = selection
            && matches!(op, sqlparser::ast::BinaryOperator::Eq)
        {
            let (col_name, val_expr) = match (left.as_ref(), right.as_ref()) {
                (SqlExpr::Identifier(id), SqlExpr::Value(v)) => (id.value.clone(), v),
                (SqlExpr::Value(v), SqlExpr::Identifier(id)) => (id.value.clone(), v),
                (SqlExpr::CompoundIdentifier(parts), SqlExpr::Value(v)) => {
                    (parts.last()?.value.clone(), v)
                }
                (SqlExpr::Value(v), SqlExpr::CompoundIdentifier(parts)) => {
                    (parts.last()?.value.clone(), v)
                }
                _ => return None,
            };
            if let Ok(value) = SqlParser::map_value(&val_expr.value) {
                return Some((col_name, value));
            }
        }
        None
    }

    /// Use index if available, otherwise fall back to column scan.
    fn lookup_indexed_or_scan(
        table: &crate::core::table::Table,
        column_name: &str,
        value: &Value,
    ) -> Vec<crate::core::table::Row> {
        let mut lookup_val = value.clone();
        if let Value::String(s) = value
            && let Some(id) = table.string_pool.get_id(s.as_str())
        {
            lookup_val = Value::InternedString(id);
        }
        if let Some(col_idx) = table.column_map.get(column_name) {
            if let Some(index) = table.indexes.values().find(|idx| idx.col_idx == *col_idx) {
                if index.is_unique {
                    return index
                        .unique_map
                        .get(&lookup_val)
                        .map(|&i| vec![table.get_row_by_index(i)])
                        .unwrap_or_default();
                } else {
                    return index
                        .map
                        .get(&lookup_val)
                        .map(|ids| ids.iter().filter_map(|id| table.get(id)).collect())
                        .unwrap_or_default();
                }
            }
        }
        table.find_by_column(column_name, &lookup_val)
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

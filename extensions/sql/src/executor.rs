use std::cell::RefCell;

use dbobj::{ColumnDefinition, Database, RowData, Schema, Value};
use crate::local_parser::{
    AlterOperation, Assignment, Expr, Join, Parser as LocalParser, Statement,
};
use compact_str::CompactString;

const MAX_CACHE_SIZE: usize = 512;

pub type StatementCache = dbobj::FastHashMap<String, Vec<Statement>>;

#[derive(Clone)]
pub struct PreparedStatement {
    statements: Vec<Statement>,
    param_count: usize,
}

impl PreparedStatement {
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
        Statement::Select {
            selection: Some(sel),
            ..
        } => {
            count_expr_placeholders(sel, count);
        }
        Statement::Select {
            selection: None, ..
        } => {}
        Statement::Insert { values, .. } => {
            for row in values {
                for val in row {
                    count_expr_placeholders(val, count);
                }
            }
        }
        Statement::Update {
            assignments,
            selection,
            ..
        } => {
            for assign in assignments {
                count_expr_placeholders(&assign.value, count);
            }
            if let Some(sel) = selection {
                count_expr_placeholders(sel, count);
            }
        }
        Statement::Delete {
            selection: Some(sel),
            ..
        } => {
            count_expr_placeholders(sel, count);
        }
        Statement::Delete {
            selection: None, ..
        } => {}
        _ => {}
    }
}

fn count_expr_placeholders(expr: &Expr, count: &mut usize) {
    match expr {
        Expr::Placeholder => {
            *count += 1;
        }
        Expr::Binary(left, _, right) => {
            count_expr_placeholders(left, count);
            count_expr_placeholders(right, count);
        }
        Expr::Nested(inner) => {
            count_expr_placeholders(inner, count);
        }
        _ => {}
    }
}

fn substitute_statement(stmt: &Statement, params: &[Value], idx: &mut usize) -> Statement {
    match stmt {
        Statement::Select {
            columns,
            table,
            selection,
            join,
        } => Statement::Select {
            columns: columns.clone(),
            table: table.clone(),
            selection: selection.as_ref().map(|s| substitute_expr(s, params, idx)),
            join: join.clone(),
        },
        Statement::Insert {
            table,
            columns,
            values,
        } => {
            let new_values: Vec<Vec<Expr>> = values
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|val| substitute_expr(val, params, idx))
                        .collect()
                })
                .collect();
            Statement::Insert {
                table: table.clone(),
                columns: columns.clone(),
                values: new_values,
            }
        }
        Statement::Update {
            table,
            assignments,
            selection,
        } => {
            let new_assignments: Vec<Assignment> = assignments
                .iter()
                .map(|a| Assignment {
                    column: a.column.clone(),
                    value: substitute_expr(&a.value, params, idx),
                })
                .collect();
            Statement::Update {
                table: table.clone(),
                assignments: new_assignments,
                selection: selection.as_ref().map(|s| substitute_expr(s, params, idx)),
            }
        }
        Statement::Delete { table, selection } => Statement::Delete {
            table: table.clone(),
            selection: selection.as_ref().map(|s| substitute_expr(s, params, idx)),
        },
        _ => stmt.clone(),
    }
}

fn substitute_expr(expr: &Expr, params: &[Value], idx: &mut usize) -> Expr {
    match expr {
        Expr::Placeholder => {
            let val = params[*idx].clone();
            *idx += 1;
            Expr::Literal(val)
        }
        Expr::Binary(left, op, right) => Expr::Binary(
            Box::new(substitute_expr(left, params, idx)),
            op.clone(),
            Box::new(substitute_expr(right, params, idx)),
        ),
        Expr::Nested(inner) => Expr::Nested(Box::new(substitute_expr(inner, params, idx))),
        _ => expr.clone(),
    }
}

pub struct SqlExecutor<'a> {
    db: &'a Database,
    stmt_cache: RefCell<StatementCache>,
    populated: RefCell<Vec<String>>,
}

impl<'a> SqlExecutor<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self {
            db,
            stmt_cache: RefCell::new(dbobj::FastHashMap::default()),
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
                let mut parser = LocalParser::new(sql);
                let parsed = parser.parse_statements().map_err(|e| e.to_string())?;
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

    pub fn prepare(&self, sql: &str) -> Result<PreparedStatement, String> {
        let statements = {
            let cache = self.stmt_cache.borrow();
            if let Some(cached) = cache.get(sql) {
                cached.clone()
            } else {
                drop(cache);
                let mut parser = LocalParser::new(sql);
                let parsed = parser.parse_statements().map_err(|e| e.to_string())?;
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
        let mut parser = LocalParser::new(sql);
        let parsed = parser.parse_statements().map_err(|e| e.to_string())?;
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
            let id_pairs: Vec<_> = table
                .ids
                .iter()
                .enumerate()
                .map(|(i, id)| (i, id.clone()))
                .collect();
            for (i, id) in id_pairs {
                table.id_map.insert(id, i);
            }
        }
        self.populated.borrow_mut().push(table_name.to_string());
        Ok(())
    }

    fn try_extract_id_filter(selection: &Expr, has_id_column: bool) -> Option<dbobj::Id> {
        if has_id_column {
            return None;
        }
        if let Expr::Binary(left, dbobj::Operator::Eq, right) = selection {
            let val_expr = match (left.as_ref(), right.as_ref()) {
                (Expr::Column(id), val) if id.as_str() == "id" => val,
                (val, Expr::Column(id)) if id.as_str() == "id" => val,
                (Expr::CompoundColumn(_, col), val) if col.as_str() == "id" => val,
                (val, Expr::CompoundColumn(_, col)) if col.as_str() == "id" => val,
                _ => return None,
            };
            if let Expr::Literal(Value::Integer(n)) = val_expr {
                return Some(dbobj::Id::Integer(*n as u64));
            }
        }
        None
    }

    fn execute_statement(&self, stmt: Statement) -> Result<SqlResult, String> {
        match stmt {
            Statement::CreateTable { name, columns } => {
                let mut has_id = false;
                let col_defs: Vec<ColumnDefinition> = columns
                    .into_iter()
                    .map(|col| {
                        if col.name.as_str() == "id" {
                            has_id = true;
                        }
                        ColumnDefinition {
                            name: col.name,
                            data_type: col.data_type,
                            nullable: true,
                        }
                    })
                    .collect();
                let schema = Schema { columns: col_defs };
                let table_name = name.to_string();
                self.db.create_table(table_name.clone(), schema);
                if has_id {
                    let _ = self.db.create_unique_index(&table_name, "id");
                }
                Ok(SqlResult::Ok)
            }
            Statement::Insert {
                table,
                columns,
                values,
            } => {
                let table_name_str = table.to_string();
                let num_rows = values.len();

                if num_rows > 1 {
                    let col_indices: Vec<usize>;
                    let num_cols: usize;
                    {
                        let table_lock = self
                            .db
                            .get_table(&table_name_str)
                            .ok_or_else(|| format!("Table {} not found", table_name_str))?;
                        let table_ref = table_lock.read();
                        num_cols = table_ref.num_columns;
                        col_indices = if columns.is_empty() {
                            (0..num_cols).collect()
                        } else {
                            columns
                                .iter()
                                .map(|col| {
                                    table_ref.column_map.get(col.as_str()).copied().ok_or_else(
                                        || {
                                            format!(
                                                "Column {} not found in table {}",
                                                col, table_name_str
                                            )
                                        },
                                    )
                                })
                                .collect::<Result<Vec<_>, _>>()?
                        };
                    }

                    let mut batch: Vec<Vec<Value>> = Vec::with_capacity(num_rows);
                    for row_values in values {
                        let mut vals = vec![Value::Null; num_cols];
                        for (i, val_expr) in row_values.into_iter().enumerate() {
                            if let Expr::Literal(value) = val_expr {
                                vals[col_indices[i]] = value;
                            }
                        }
                        batch.push(vals);
                    }
                    self.db
                        .insert_batch_values(&table_name_str, batch)
                        .map_err(|e| e.to_string())?;
                } else {
                    let mut row_data = RowData::default();
                    {
                        let table_lock = if columns.is_empty() {
                            Some(
                                self.db
                                    .get_table(&table_name_str)
                                    .ok_or_else(|| format!("Table {} not found", table_name_str))?,
                            )
                        } else {
                            None
                        };
                        let table_guard = table_lock.as_ref().map(|l| l.read());
                        let row_values = values.into_iter().next().unwrap_or_default();

                        for (i, val_expr) in row_values.into_iter().enumerate() {
                            if let Expr::Literal(value) = val_expr {
                                if i < columns.len() {
                                    row_data.insert(columns[i].clone(), value);
                                } else if let Some(table) = &table_guard
                                    && i < table.schema.columns.len()
                                {
                                    row_data.insert(table.schema.columns[i].name.clone(), value);
                                }
                            }
                        }
                    }
                    self.db
                        .insert_row(&table_name_str, row_data, None)
                        .map_err(|e| e.to_string())?;
                }
                Ok(SqlResult::Ok)
            }
            Statement::Update {
                table,
                assignments,
                selection,
            } => {
                let table_name = table.to_string();
                self.populate_id_map_once(&table_name)?;

                let (ids, rows_data): (Vec<_>, Vec<_>) = {
                    let table_lock = self
                        .db
                        .get_table(&table_name)
                        .ok_or_else(|| format!("Table {} not found", table_name))?;
                    let table_ref = table_lock.read();
                    let has_id_column = table_ref.column_map.contains_key("id");
                    let rows_to_update = if let Some(direct_id) = selection
                        .as_ref()
                        .and_then(|s| Self::try_extract_id_filter(s, has_id_column))
                    {
                        if let Some(row) = table_ref.get(&direct_id) {
                            vec![row]
                        } else {
                            Vec::new()
                        }
                    } else if let Some(selection) = &selection {
                        if let Some((col_name, value)) = try_extract_eq_literal(selection) {
                            lookup_indexed_or_scan(&table_ref, &col_name, &value)
                        } else {
                            let mapped = map_expr_to_core(selection)?;
                            let cap = table_ref.column_map.len() * 2 + 1;
                            let mut mapping = dbobj::FastHashMap::with_capacity_and_hasher(
                                cap,
                                Default::default(),
                            );
                            for (col, idx) in &table_ref.column_map {
                                mapping.insert(col.clone(), *idx);
                                mapping.insert(format!("{}.{}", table_name, col), *idx);
                            }
                            if !mapping.contains_key("id") {
                                mapping.insert("id".to_string(), usize::MAX);
                            }
                            table_ref.select(|r| mapped.is_true(r, &mapping, &table_ref))
                        }
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
                    for assignment in &assignments {
                        let col_name = assignment.column.to_string();
                        if let Expr::Literal(v) = &assignment.value {
                            row_data.insert(CompactString::from(col_name), v.clone());
                        }
                    }
                    self.db
                        .update_row(&table_name, &id, row_data)
                        .map_err(|e| e.to_string())?;
                }
                Ok(SqlResult::Ok)
            }
            Statement::Delete { table, selection } => {
                let table_name = table.to_string();
                self.populate_id_map_once(&table_name)?;

                let ids: Vec<_> = {
                    let table_lock = self
                        .db
                        .get_table(&table_name)
                        .ok_or_else(|| format!("Table {} not found", table_name))?;
                    let table_ref = table_lock.read();
                    let has_id_column = table_ref.column_map.contains_key("id");
                    let rows_to_delete = if let Some(direct_id) = selection
                        .as_ref()
                        .and_then(|s| Self::try_extract_id_filter(s, has_id_column))
                    {
                        if let Some(row) = table_ref.get(&direct_id) {
                            vec![row]
                        } else {
                            Vec::new()
                        }
                    } else if let Some(selection) = &selection {
                        if let Some((col_name, value)) = try_extract_eq_literal(selection) {
                            lookup_indexed_or_scan(&table_ref, &col_name, &value)
                        } else {
                            let mapped = map_expr_to_core(selection)?;
                            let cap = table_ref.column_map.len() * 2 + 1;
                            let mut mapping = dbobj::FastHashMap::with_capacity_and_hasher(
                                cap,
                                Default::default(),
                            );
                            for (col, idx) in &table_ref.column_map {
                                mapping.insert(col.clone(), *idx);
                                mapping.insert(format!("{}.{}", table_name, col), *idx);
                            }
                            if !mapping.contains_key("id") {
                                mapping.insert("id".to_string(), usize::MAX);
                            }
                            table_ref.select(|r| mapped.is_true(r, &mapping, &table_ref))
                        }
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
            Statement::AlterTable { name, operation } => {
                let table_name = name.to_string();
                let AlterOperation::AddColumn(col_def) = operation;
                let table_lock = self
                    .db
                    .get_table(&table_name)
                    .ok_or_else(|| format!("Table {} not found", table_name))?;
                let mut table = table_lock.write();

                let col_name = col_def.name;
                let data_type = col_def.data_type;

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
                Ok(SqlResult::Ok)
            }
            Statement::Select {
                columns: _,
                table,
                selection,
                join,
            } => self.execute_query(&table, &selection, join.as_ref()),
        }
    }

    fn execute_query(
        &self,
        table_name: &CompactString,
        selection: &Option<Expr>,
        join: Option<&Join>,
    ) -> Result<SqlResult, String> {
        let table_name_str = table_name.to_string();

        if let Some(join) = join {
            return self.execute_join(&table_name_str, join);
        }

        let table_lock = self
            .db
            .get_table(&table_name_str)
            .ok_or_else(|| format!("Table {} not found", table_name_str))?;
        let table_ref = table_lock.read();

        let has_id_column = table_ref.column_map.contains_key("id");
        let fast_id = selection
            .as_ref()
            .and_then(|s| Self::try_extract_id_filter(s, has_id_column));

        let already_populated = self.populated.borrow().contains(&table_name_str);

        if fast_id.is_some() && !already_populated {
            drop(table_ref);
            drop(table_lock);
            self.populate_id_map_once(&table_name_str)?;
            let table_lock = self
                .db
                .get_table(&table_name_str)
                .ok_or_else(|| format!("Table {} not found", table_name_str))?;
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
        } else if let Some(selection) = selection {
            if let Some((col_name, value)) = try_extract_eq_literal(selection) {
                lookup_indexed_or_scan(&table_ref, &col_name, &value)
            } else {
                let mapped = map_expr_to_core(selection)?;
                let cap = table_ref.column_map.len() * 2 + 1;
                let mut mapping =
                    dbobj::FastHashMap::with_capacity_and_hasher(cap, Default::default());
                for (col, idx) in &table_ref.column_map {
                    mapping.insert(col.clone(), *idx);
                    mapping.insert(format!("{}.{}", table_name_str, col), *idx);
                }
                if !mapping.contains_key("id") {
                    mapping.insert("id".to_string(), usize::MAX);
                }
                table_ref.select(|r| mapped.is_true(r, &mapping, &table_ref))
            }
        } else {
            (0..table_ref.ids.len())
                .map(|i| table_ref.get_row_by_index(i))
                .collect()
        };

        let results = rows.into_iter().map(|r| r.to_map(&table_ref)).collect();
        Ok(SqlResult::Rows(results))
    }

    fn execute_join(&self, table1_name: &str, join: &Join) -> Result<SqlResult, String> {
        let table2_name = join.table.to_string();

        let joined_rows = self
            .db
            .hash_join(
                table1_name,
                join.left_col.as_str(),
                &table2_name,
                join.right_col.as_str(),
            )
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
                combined.insert(CompactString::from(format!("{}.{}", table1_name, k)), v);
            }
            for (k, v) in m2 {
                combined.insert(CompactString::from(format!("{}.{}", table2_name, k)), v);
            }
            results.push(combined);
        }
        Ok(SqlResult::Rows(results))
    }
}

fn try_extract_eq_literal(selection: &Expr) -> Option<(String, Value)> {
    if let Expr::Binary(left, dbobj::Operator::Eq, right) = selection {
        let (col_name, val_expr) = match (left.as_ref(), right.as_ref()) {
            (Expr::Column(id), Expr::Literal(v)) => (id.to_string(), v),
            (Expr::Literal(v), Expr::Column(id)) => (id.to_string(), v),
            (Expr::CompoundColumn(_, col), Expr::Literal(v)) => (col.to_string(), v),
            (Expr::Literal(v), Expr::CompoundColumn(_, col)) => (col.to_string(), v),
            _ => return None,
        };
        return Some((col_name, val_expr.clone()));
    }
    None
}

fn lookup_indexed_or_scan(
    table: &dbobj::table::Table,
    column_name: &str,
    value: &Value,
) -> Vec<dbobj::table::Row> {
    let mut lookup_val = value.clone();
    if let Value::String(s) = value
        && let Some(id) = table.string_pool.get_id(s.as_str())
    {
        lookup_val = Value::InternedString(id);
    }
    if let Some(col_idx) = table.column_map.get(column_name)
        && let Some(index) = table.indexes.values().find(|idx| idx.col_idx == *col_idx)
    {
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
    table.find_by_column(column_name, &lookup_val)
}

fn map_expr_to_core(expr: &Expr) -> Result<dbobj::Expr, String> {
    match expr {
        Expr::Literal(v) => Ok(dbobj::Expr::Literal(v.clone())),
        Expr::Column(name) => Ok(dbobj::Expr::Column(name.clone())),
        Expr::CompoundColumn(_, col_name) => Ok(dbobj::Expr::Column(col_name.clone())),
        Expr::Placeholder => Err("Unexpected placeholder in expression evaluation".to_string()),
        Expr::Binary(left, op, right) => {
            let l = Box::new(map_expr_to_core(left)?);
            let r = Box::new(map_expr_to_core(right)?);
            Ok(dbobj::Expr::Binary(l, op.clone(), r))
        }
        Expr::Nested(inner) => Ok(map_expr_to_core(inner)?),
    }
}

#[derive(Debug)]
pub enum SqlResult {
    Ok,
    Rows(Vec<RowData>),
}

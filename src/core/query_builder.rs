use super::database::Database;
use super::query::Expr;
use super::table::Row;
use super::value::Value;
use super::Id;

#[derive(PartialEq)]
pub enum QueryType {
    Select,
    Insert,
    Update,
    Delete,
}

pub struct QueryBuilder {
    query_type: QueryType,
    table: String,
    columns: Option<Vec<String>>,
    condition: Option<Expr>,
    order_column: Option<String>,
    order_desc: bool,
    limit: Option<usize>,
    offset: Option<usize>,
    set_values: Vec<(String, Value)>,
    join_table: Option<String>,
    join_col1: Option<String>,
    join_col2: Option<String>,
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self::select("")
    }
}

impl QueryBuilder {
    fn new(query_type: QueryType, table: impl Into<String>) -> Self {
        Self {
            query_type,
            table: table.into(),
            columns: None,
            condition: None,
            order_column: None,
            order_desc: false,
            limit: None,
            offset: None,
            set_values: Vec::new(),
            join_table: None,
            join_col1: None,
            join_col2: None,
        }
    }

    pub fn select(table: impl Into<String>) -> Self {
        Self::new(QueryType::Select, table)
    }

    pub fn insert(table: impl Into<String>) -> Self {
        Self::new(QueryType::Insert, table)
    }

    pub fn update(table: impl Into<String>) -> Self {
        Self::new(QueryType::Update, table)
    }

    pub fn delete(table: impl Into<String>) -> Self {
        Self::new(QueryType::Delete, table)
    }

    pub fn columns(mut self, columns: Vec<impl Into<String>>) -> Self {
        self.columns = Some(columns.into_iter().map(|c| c.into()).collect());
        self
    }

    pub fn r#where(mut self, expr: Expr) -> Self {
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn where_eq(mut self, column: impl Into<String>, value: Value) -> Self {
        let expr = Expr::col(column.into()).eq(Expr::lit(value));
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn where_neq(mut self, column: impl Into<String>, value: Value) -> Self {
        let expr = Expr::col(column.into()).neq(Expr::lit(value));
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn where_gt(mut self, column: impl Into<String>, value: Value) -> Self {
        let expr = Expr::col(column.into()).gt(Expr::lit(value));
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn where_gte(mut self, column: impl Into<String>, value: Value) -> Self {
        let expr = Expr::col(column.into()).gte(Expr::lit(value));
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn where_lt(mut self, column: impl Into<String>, value: Value) -> Self {
        let expr = Expr::col(column.into()).lt(Expr::lit(value));
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn where_lte(mut self, column: impl Into<String>, value: Value) -> Self {
        let expr = Expr::col(column.into()).lte(Expr::lit(value));
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn where_like(mut self, column: impl Into<String>, pattern: impl Into<String>) -> Self {
        let expr = Expr::col(column.into()).like(Expr::lit(Value::String(pattern.into().into())));
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn where_or(mut self, left: Expr, right: Expr) -> Self {
        let expr = left.or(right);
        self.condition = Some(match self.condition.take() {
            Some(existing) => existing.and(expr),
            None => expr,
        });
        self
    }

    pub fn order_by(mut self, column: impl Into<String>, descending: bool) -> Self {
        self.order_column = Some(column.into());
        self.order_desc = descending;
        self
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }

    pub fn set(mut self, column: impl Into<String>, value: Value) -> Self {
        self.set_values.push((column.into(), value));
        self
    }

    pub fn join(
        mut self,
        table: impl Into<String>,
        col1: impl Into<String>,
        col2: impl Into<String>,
    ) -> Self {
        self.join_table = Some(table.into());
        self.join_col1 = Some(col1.into());
        self.join_col2 = Some(col2.into());
        self
    }

    /// Returns true if this is a simple SELECT * with no filters, joins, order, or pagination.
    pub fn is_simple_select(&self) -> bool {
        self.query_type == QueryType::Select
            && self.condition.is_none()
            && self.order_column.is_none()
            && self.limit.is_none()
            && self.offset.is_none()
            && self.columns.is_none()
            && self.join_table.is_none()
    }

    pub fn table_name(&self) -> &str {
        &self.table
    }

    pub fn run(&self, db: &Database) -> Result<Vec<Row>, String> {
        match self.query_type {
            QueryType::Select => self.run_select(db),
            QueryType::Insert => self.run_insert(db),
            QueryType::Update => self.run_update(db),
            QueryType::Delete => self.run_delete(db),
        }
    }

    pub fn run_first(&self, db: &Database) -> Result<Option<Row>, String> {
        let mut results = self.run(db)?;
        Ok(if results.is_empty() {
            None
        } else {
            Some(results.remove(0))
        })
    }

    fn run_select(&self, db: &Database) -> Result<Vec<Row>, String> {
        if let Some(ref join_table) = self.join_table {
            return self.run_select_join(db, join_table);
        }

        let tables_guard = db.tables.read();
        let table_lock = tables_guard
            .get(&self.table)
            .ok_or_else(|| format!("Table '{}' not found", self.table))?;
        let table = table_lock.read();

        // Resolve interned strings in the returned data
        let results = if let Some(ref expr) = self.condition {
            let plan = expr.plan(&table);
            match plan {
                super::query::QueryPlan::FullScan(_, expr) => {
                    table.select(|r| expr.is_true(r, &table.column_map, &table))
                }
                super::query::QueryPlan::IndexScan(_, col, val) => table.find_by_column(&col, &val),
                super::query::QueryPlan::IndexFilteredScan(_, col, val, expr) => {
                    let candidates = table.find_by_column(&col, &val);
                    candidates
                        .into_iter()
                        .filter(|r| expr.is_true(r, &table.column_map, &table))
                        .collect()
                }
            }
        } else {
            let num_rows = table.ids.len();
            (0..num_rows).map(|i| table.get_row_by_index(i)).collect()
        };

        let results = self.apply_projections(results, &table);
        let results = self.apply_order(results, &table);
        let results = self.apply_pagination(results);

        let results = results
            .into_iter()
            .map(|row| Self::resolve_row(&row, &table))
            .collect();

        Ok(results)
    }

    fn run_select_join(&self, db: &Database, join_table: &str) -> Result<Vec<Row>, String> {
        let col1 = self
            .join_col1
            .as_ref()
            .ok_or("Join column 1 not specified")?;
        let col2 = self
            .join_col2
            .as_ref()
            .ok_or("Join column 2 not specified")?;

        // Use hash_join_indices to avoid creating right-side Row objects (discarded below)
        let indices = db
            .hash_join_indices(&self.table, col1, join_table, col2)
            .map_err(|e| e.to_string())?;

        let tables_guard = db.tables.read();
        let table_lock = tables_guard
            .get(&self.table)
            .ok_or_else(|| format!("Table '{}' not found", self.table))?;
        let table = table_lock.read();

        let mut results: Vec<Row> = Vec::with_capacity(indices.len());
        for (t1_idx, _t2_idx) in indices {
            let r1 = table.get_row_by_index(t1_idx);
            if let Some(ref expr) = self.condition {
                if !expr.is_true(&r1, &table.column_map, &table) {
                    continue;
                }
            }
            results.push(r1);
        }

        let results = self.apply_order(results, &table);
        let results = self.apply_pagination(results);

        let results = results
            .into_iter()
            .map(|row| Self::resolve_row(&row, &table))
            .collect();
        Ok(results)
    }

    fn resolve_row(row: &Row, table: &super::Table) -> Row {
        let resolved_data: Vec<Value> = row
            .data
            .iter()
            .map(|v| {
                if let Value::InternedString(id) = v {
                    table
                        .string_pool
                        .resolve(*id)
                        .map(|s| Value::String(s))
                        .unwrap_or_else(|| v.clone())
                } else {
                    v.clone()
                }
            })
            .collect();
        Row {
            id: row.id.clone(),
            data: resolved_data.into(),
            version: row.version,
        }
    }

    fn run_insert(&self, db: &Database) -> Result<Vec<Row>, String> {
        let id = db
            .insert_values(&self.table, self.build_insert_values(db))
            .map_err(|e| e.to_string())?;
        let tables_guard = db.tables.read();
        let table_lock = tables_guard
            .get(&self.table)
            .ok_or_else(|| format!("Table '{}' not found", self.table))?;
        let table = table_lock.read();
        let row = table.get(&id);
        Ok(row
            .map(|r| Self::resolve_row(&r, &table))
            .into_iter()
            .collect())
    }

    fn run_update(&self, db: &Database) -> Result<Vec<Row>, String> {
        let matching = self.run_select(db)?;
        let ids: Vec<Id> = matching.iter().map(|r| r.id.clone()).collect();

        for row in &matching {
            let tables_guard = db.tables.read();
            let table_lock = tables_guard
                .get(&self.table)
                .ok_or_else(|| format!("Table '{}' not found", self.table))?;
            let table_read = table_lock.read();

            let row_idx = table_read.get_index(&row.id).unwrap_or(0);
            let mut new_values = Vec::with_capacity(table_read.num_columns);
            for (col_idx, col_def) in table_read.schema.columns.iter().enumerate() {
                let existing = &table_read.data[row_idx * table_read.num_columns + col_idx];
                match self
                    .set_values
                    .iter()
                    .find(|(name, _)| name == &col_def.name)
                {
                    Some((_, val)) => new_values.push(val.clone()),
                    None => new_values.push(existing.clone()),
                }
            }
            drop(table_read);
            drop(tables_guard);

            db.update_values(&self.table, &row.id, new_values)
                .map_err(|e| e.to_string())?;
        }

        // Re-read updated rows
        let tables_guard = db.tables.read();
        let table_lock = tables_guard
            .get(&self.table)
            .ok_or_else(|| format!("Table '{}' not found", self.table))?;
        let table = table_lock.read();

        let updated: Vec<Row> = ids
            .iter()
            .filter_map(|id| {
                let idx = table.get_index(id)?;
                Some(Self::resolve_row(&table.get_row_by_index(idx), &table))
            })
            .collect();

        Ok(updated)
    }

    fn run_delete(&self, db: &Database) -> Result<Vec<Row>, String> {
        let matching = self.run_select(db)?;
        let ids: Vec<Id> = matching.iter().map(|r| r.id.clone()).collect();

        db.delete_batch(&self.table, &ids)
            .map_err(|e| e.to_string())?;

        Ok(matching)
    }

    fn build_insert_values(&self, db: &Database) -> Vec<Value> {
        let tables_guard = db.tables.read();
        let table_lock = tables_guard.get(&self.table).unwrap();
        let table = table_lock.read();

        let mut values = vec![Value::Null; table.num_columns];
        for (col_name, val) in &self.set_values {
            if let Some(&idx) = table.column_map.get(col_name) {
                values[idx] = val.clone();
            }
        }
        values
    }

    fn apply_projections(&self, rows: Vec<Row>, table: &super::Table) -> Vec<Row> {
        let columns = match self.columns {
            Some(ref cols) => cols.clone(),
            None => return rows,
        };

        rows.into_iter()
            .map(|row| {
                let row_idx = table.get_index(&row.id).unwrap_or(0);
                let mut new_data = Vec::with_capacity(columns.len());
                for col_name in &columns {
                    if col_name == "id" {
                        continue;
                    }
                    if let Some(&idx) = table.column_map.get(col_name) {
                        new_data.push(table.data[row_idx * table.num_columns + idx].clone());
                    }
                }
                Row {
                    id: row.id,
                    data: new_data.into(),
                    version: row.version,
                }
            })
            .collect()
    }

    fn apply_order(&self, mut rows: Vec<Row>, table: &super::Table) -> Vec<Row> {
        let col_name = match self.order_column {
            Some(ref c) => c.clone(),
            None => return rows,
        };

        let col_idx = match table.column_map.get(&col_name) {
            Some(&idx) => idx,
            None => return rows,
        };

        let desc = self.order_desc;
        rows.sort_by(|a, b| {
            let a_idx = table.get_index(&a.id).unwrap_or(0);
            let b_idx = table.get_index(&b.id).unwrap_or(0);
            let a_val = &table.data[a_idx * table.num_columns + col_idx];
            let b_val = &table.data[b_idx * table.num_columns + col_idx];
            if desc {
                b_val.cmp(a_val)
            } else {
                a_val.cmp(b_val)
            }
        });
        rows
    }

    fn apply_pagination(&self, rows: Vec<Row>) -> Vec<Row> {
        let start = self.offset.unwrap_or(0);
        let count = self.limit.unwrap_or(usize::MAX);

        if start >= rows.len() {
            Vec::new()
        } else {
            let end = (start + count).min(rows.len());
            rows[start..end].to_vec()
        }
    }
}

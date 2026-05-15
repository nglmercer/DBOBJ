# SQL Reference

Complete grammar reference for the DBOBJ embedded SQL engine.

---

## Executor Entry Points

| JavaScript API | Description |
|----------------|-------------|
| `db.executeSql(sql)` | Execute one statement; returns result or `"OK"` |
| `db.query(sql, params?)` | Prepare with `?` placeholders; returns `PreparedStatement` |
| `db.prepare(sql, params?)` | Same as `query`; compile for reuse |
| `db.queryI64(sql)` | SELECT first column as `BigInt64Array` |
| `db.queryJoinI64(sql)` | SELECT join result as flat ID-pair array |

---

## Supported Data Types

| SQL keyword | `DataType` | JS type |
|-------------|------------|---------|
| `INTEGER`, `INT`, `BIGINT` | `Integer` | `number` / `bigint` |
| `FLOAT`, `DOUBLE`, `REAL` | `Float` | `number` |
| `TEXT`, `STRING`, `VARCHAR(n)`, `CHAR(n)` | `String` | `string` |
| `BOOLEAN`, `BOOL` | `Boolean` | `boolean` |
| `BLOB` | `Blob` | Buffer / ArrayBuffer |

Case-insensitive. Parenthesised length specifiers on `VARCHAR` / `CHAR` are parsed but
ignored.

---

## DDL — CREATE TABLE

### Syntax

```sql
CREATE TABLE [IF NOT EXISTS] table_name (
  column_name DATA_TYPE [NOT NULL] [DEFAULT expr],
  column_name DATA_TYPE [NOT NULL] [DEFAULT expr],
  ...
);
```

### Features

| Feature | Description |
|---------|-------------|
| `IF NOT EXISTS` | Silently succeeds if the table already exists |
| `NOT NULL` | Rejects `NULL` values on insert |
| `DEFAULT expr` | Supplies a fallback value when the column is omitted on insert |

### Examples

```sql
CREATE TABLE users (
  id      INTEGER     NOT NULL,
  name    TEXT                  DEFAULT 'guest',
  email   TEXT        NOT NULL,
  age     INTEGER,
  active  BOOLEAN               DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS logs (
  id    INTEGER   NOT NULL,
  ts    FLOAT              DEFAULT 0.0,
  level TEXT,
  msg   TEXT
);
```

### Column Default Expressions

Supported default expression forms:

| Expression | Example |
|------------|---------|
| Integer literal | `DEFAULT 0` |
| Float literal | `DEFAULT 3.14` |
| String literal | `DEFAULT 'guest'` |
| Boolean literal | `DEFAULT TRUE` / `DEFAULT FALSE` |
| `NULL` | `DEFAULT NULL` |

---

## DDL — ALTER TABLE

### ADD COLUMN

```sql
ALTER TABLE table_name ADD [COLUMN] column_name DATA_TYPE [NOT NULL] [DEFAULT expr];
```

The `COLUMN` keyword is optional.

```sql
ALTER TABLE users ADD COLUMN age INTEGER;
ALTER TABLE users ADD age INTEGER DEFAULT 0;
```

Existing rows receive `NULL` (or the `DEFAULT`) for the new column.

---

## DDL — DROP TABLE

```sql
DROP TABLE [IF EXISTS] table_name;
```

```sql
DROP TABLE users;
DROP TABLE IF EXISTS users; -- no error if not found
```

---

## DML — INSERT

### Syntax

```sql
INSERT INTO table_name [(column_list)]
  VALUES (row_values), (row_values), ...;
```

### Named-column form

```sql
INSERT INTO users (name, age) VALUES ('Alice', 30);
```

Unspecified columns receive their `DEFAULT` or `NULL`.

### Positional form

```sql
INSERT INTO users VALUES (1, 'Alice', 30, true);
```

All columns must be supplied in schema order.

### Multi-row insert

```sql
INSERT INTO users (name, age) VALUES
  ('Alice', 30),
  ('Bob',   25),
  ('Carol', 35);
```

Supports up to thousands of rows in one statement.

---

## DML — UPDATE

```sql
UPDATE table_name
  SET column = expr [, column = expr ...]
  [WHERE condition];
```

Omitting `WHERE` updates every row in the table.

```sql
-- Update specific columns
UPDATE users SET age = 31 WHERE name = 'Alice';

-- Update multiple columns
UPDATE users SET score = 99, active = true WHERE id = 1;

-- Update all rows
UPDATE products SET price = price * 0.9;
```

---

## DML — DELETE

```sql
DELETE FROM table_name [WHERE condition];
```

Omitting `WHERE` deletes every row in the table.

```sql
DELETE FROM users WHERE id = 1;
DELETE FROM sessions WHERE expired = true;
```

---

## SELECT

### Syntax

```sql
SELECT
  [DISTINCT] select_list
FROM table_name
  [INNER JOIN table2 ON table1.col = table2.col]
  [WHERE condition]
  [GROUP BY column_list]
  [HAVING condition]
  [ORDER BY column [ASC | DESC]]
  [LIMIT n [OFFSET m]];
```

### Column Selection

```sql
SELECT * FROM users;              -- all columns
SELECT id, name FROM users;       -- specific columns
```

---

### WHERE Clause

#### Comparison Operators

| Operator | Description |
|----------|-------------|
| `=` | Equal |
| `!=` / `<>` | Not equal |
| `<` | Less than |
| `<=` | Less than or equal |
| `>` | Greater than |
| `>=` | Greater than or equal |

#### Logical Operators

| Operator | Description |
|----------|-------------|
| `AND` | Both sides must be true |
| `OR` | Either side must be true |
| `NOT` | Negate a condition |

`NOT` has the highest precedence, `AND` binds tighter than `OR`. Parentheses
override precedence.

```sql
SELECT * FROM users WHERE age >= 18 AND active = true;
SELECT * FROM users WHERE role = 'admin' OR role = 'moderator';
SELECT * FROM t WHERE (a = 1 OR b = 2) AND c = 3;
```

#### NULL Test

```sql
SELECT * FROM users WHERE deleted_at IS NULL;
SELECT * FROM users WHERE deleted_at IS NOT NULL;
```

#### LIKE Pattern Matching

```sql
SELECT * FROM users WHERE name LIKE 'A%';      -- starts with "A"
SELECT * FROM users WHERE email LIKE '%@gmail.com'; -- ends with "@gmail.com"
SELECT * FROM users WHERE name LIKE '%son%';   -- contains "son"
SELECT * FROM users WHERE name LIKE '___';     -- exactly 3 characters
```

| Wildcard | Meaning |
|----------|---------|
| `%` | Any sequence of zero or more characters |
| `_` | Exactly one character |

ILIKE is not yet supported.

---

### JOIN

#### INNER JOIN

```sql
SELECT * FROM users
INNER JOIN orders ON users.id = orders.user_id;
```

The `INNER` keyword is optional:

```sql
SELECT * FROM users JOIN orders ON users.id = orders.user_id;
```

Result columns are prefixed with their table name to avoid ambiguity:

```
userId  name   orderId  user_id  total
1       Alice  101      1        49.99
1       Alice  102      1        19.99
2       Bob    201      2        79.00
```

---

### ORDER BY

```sql
SELECT * FROM users ORDER BY name;       -- ascending (default)
SELECT * FROM users ORDER BY name ASC;   -- explicit ascending
SELECT * FROM users ORDER BY score DESC; -- descending
```

Multiple columns:

```sql
SELECT * FROM users ORDER BY active DESC, name ASC;
```

---

### LIMIT / OFFSET

```sql
SELECT * FROM users LIMIT 10;          -- first 10 rows
SELECT * FROM users LIMIT 10 OFFSET 5; -- rows 6-15
```

Works together with ORDER BY:

```sql
SELECT * FROM users ORDER BY score DESC LIMIT 5;
```

---

### Parameter Binding

Use `?` as a placeholder in any SQL statement. Values are bound at execution time
and prevent SQL injection.

```sql
SELECT * FROM users WHERE id = ?;
INSERT INTO users (name, age) VALUES (?, ?);
UPDATE users SET score = ? WHERE id = ?;
DELETE FROM users WHERE id = ?;
```

Pass values as the second argument to `query()` / `prepare()`:

```typescript
db.query("SELECT * FROM users WHERE id = ?", [1]).get();
db.query("INSERT INTO users (name, age) VALUES (?, ?)", ["Alice", 30]).run();
```

---

## Aggregate Functions

| Function | Description | Example |
|----------|-------------|---------|
| `COUNT(*)` | Total rows | `SELECT COUNT(*) FROM users` |
| `COUNT(col)` | Total non-NULL values | `SELECT COUNT(email) FROM users` |
| `SUM(col)` | Sum of values | `SELECT SUM(score) FROM users` |
| `AVG(col)` | Mean of values | `SELECT AVG(score) FROM users` |
| `MIN(col)` | Minimum value | `SELECT MIN(age) FROM users` |
| `MAX(col)` | Maximum value | `SELECT MAX(age) FROM users` |

Aggregate functions cannot be mixed with plain columns without `GROUP BY`.

```sql
SELECT COUNT(*), AVG(score), MAX(score) FROM users;
```

---

## Aliases

Use `AS` to name an output column:

```sql
SELECT name AS username, email AS contact FROM users;
```

---

## Multiple Statements

Pass multiple statements separated by `;` to `executeSql` or the parser. Each statement
is executed in order:

```typescript
db.executeSql(`
  CREATE TABLE users (id INTEGER, name TEXT);
  INSERT INTO users VALUES (1, 'Alice');
  INSERT INTO users VALUES (2, 'Bob');
`);
```

---

## Operator Precedence

From highest to lowest binding:

1. `()` — parenthesised expressions
2. Unary `NOT` / `-`
3. `*`, `/`
4. `+`, `-`
5. `<`, `<=`, `>`, `>=`
6. `=`, `!=`, `<>`
7. `LIKE`, `NOT LIKE`
8. `AND`
9. `OR`

---

## Error Handling

SQL method calls signal errors differently depending on the API surface:

| API | Error shape |
|-----|-------------|
| `executeSql(sql)` | Returns `DbError` with `.code` / `.message` |
| `query(sql)` | Returns `DbError` |
| `PreparedStatement.run()` | Returns `false` |
| `PreparedStatement.get()` / `.all()` | Returns `null` / `[]` on error |

```typescript
try {
  db.executeSql("SELECT * FROM nonexistent");
} catch (err: any) {
  console.error(err.code, err.message);
}
```

---

## Statement-by-Statement Reference

| Statement | Key features |
|-----------|-------------|
| `CREATE TABLE` | `IF NOT EXISTS`, `NOT NULL`, `DEFAULT` |
| `ALTER TABLE … ADD [COLUMN]` | Optional `COLUMN`, `NOT NULL`, `DEFAULT` |
| `INSERT` | Named / positional / multi-row / `?` placeholders |
| `SELECT` | `*` / column list, `WHERE`, `JOIN`, `ORDER BY`, `LIMIT`/`OFFSET`, aggregates, `LIKE`, `IS NULL`, `IS NOT NULL` |
| `UPDATE` | Multiple columns, optional `WHERE` |
| `DELETE` | Optional `WHERE` |
| `DROP TABLE` | `IF EXISTS` |

# SQL Reference

## Supported Statements

### CREATE TABLE

```sql
CREATE TABLE users (
    id INTEGER NOT NULL,
    name STRING DEFAULT 'guest',
    age INTEGER,
    active BOOLEAN
);
```

**Types:** `INTEGER`, `INT`, `BIGINT`, `FLOAT`, `DOUBLE`, `REAL`, `STRING`, `TEXT`, `VARCHAR(n)`, `CHAR(n)`, `BOOLEAN`, `BLOB`

**Constraints:** `NOT NULL`, `DEFAULT expr`

### INSERT

```sql
INSERT INTO users (id, name) VALUES (1, 'Alice');
INSERT INTO users VALUES (1, 'Alice');  -- positional
INSERT INTO users VALUES (1, 'A'), (2, 'B');  -- multi-row
```

### SELECT

```sql
SELECT * FROM users;
SELECT id, name FROM users;
SELECT COUNT(*), SUM(age), MIN(age), MAX(age) FROM users;
SELECT * FROM users WHERE name = 'Alice';
SELECT * FROM users WHERE name LIKE 'A%';
SELECT * FROM users WHERE age > 25 AND active = true;
SELECT * FROM users ORDER BY name;
SELECT * FROM users ORDER BY name DESC;
SELECT * FROM users ORDER BY id LIMIT 10;
SELECT * FROM users ORDER BY id LIMIT 10 OFFSET 20;
SELECT * FROM users INNER JOIN orders ON users.id = orders.user_id;
```

### UPDATE

```sql
UPDATE users SET age = 31 WHERE id = 1;
UPDATE users SET age = 31;  -- all rows
```

### DELETE

```sql
DELETE FROM users WHERE id = 1;
DELETE FROM users;  -- all rows
```

### ALTER TABLE

```sql
ALTER TABLE users ADD COLUMN age INTEGER;
```

### DROP TABLE

```sql
DROP TABLE users;
DROP TABLE IF EXISTS users;
```

## WHERE Operators

| Operator | Example |
|----------|---------|
| `=` | `WHERE id = 1` |
| `!=`, `<>` | `WHERE id != 1` |
| `<`, `<=`, `>`, `>=` | `WHERE age > 25` |
| `AND` | `WHERE age > 25 AND active = true` |
| `OR` | `WHERE age < 18 OR age > 65` |
| `LIKE` | `WHERE name LIKE 'A%'` |
| `()` | `WHERE (a = 1 OR b = 2) AND c = 3` |
| `?` (placeholder) | `WHERE id = ?` |

## LIKE Patterns

| Pattern | Meaning |
|---------|---------|
| `%` | Any sequence of characters |
| `_` | Any single character |
| `'A%'` | Starts with 'A' |
| `'%son%'` | Contains 'son' |
| `'___'` | Exactly 3 characters |

# DBOBJ SQL Extension

A high-performance SQL parser and execution engine specialized for the **DBOBJ** database.

## 🚀 Why a custom SQL engine?

Generic SQL parsers (like `sqlparser-rs`) are designed for compatibility, which makes them slow and heavy. **DBOBJ SQL** is a hand-written recursive descent parser that:
- Is **6x-10x faster** than generic parsers.
- Has **zero dependencies** (only `compact_str`).
- Produces native DBOBJ types directly, avoiding AST-to-Native conversion overhead.

## 🛠 Features

- **DDL**: `CREATE TABLE`, `ALTER TABLE (ADD COLUMN)`.
- **DML**: `INSERT INTO` (single and multi-value), `UPDATE`, `DELETE`.
- **Queries**: `SELECT` with `WHERE` clauses (complex boolean logic), `JOIN` (optimized Hash Join).
- **Prepared Statements**: Support for placeholders (`?`).

## 📈 Performance (Micro-benchmarks)

| Statement | LocalParser | Generic Parser | Speedup |
| :--- | :--- | :--- | :--- |
| **SELECT + JOIN** | **1.27 µs** | 13.65 µs | **10.7x** |
| **SELECT + WHERE** | **652 ns** | 6.45 µs | **9.9x** |
| **INSERT (Single)** | **1.52 µs** | 8.83 µs | **5.8x** |

## 📦 Usage (Rust)

```rust
use dbobj::Database;
use dbobj_sql::SqlExecutor;

let db = Database::new("mydb");
let executor = SqlExecutor::new(&db);

let result = executor.execute("SELECT * FROM users WHERE age > 21").unwrap();
```

## 📜 License
MIT / Apache-2.0

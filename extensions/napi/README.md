# DBOBJ N-API Bindings

High-performance native bindings for the **DBOBJ** database engine, designed for **Node.js** and **Bun**.

## Features

- **Zero-Copy Column Access** — Read database columns directly as `BigInt64Array` without serialization overhead or data cloning.
- **Embedded SQL Engine** — Execute SQL queries directly against the database with full `CREATE`, `INSERT`, `SELECT`, `UPDATE`, `DELETE` support.
- **MMap Persistence** — Instant database save/load via memory-mapped files.
- **Hash Joins** — Server-side join execution returns flat ID arrays, avoiding JS object overhead.
- **Batch Operations** — Bulk insert/update via flattened `BigInt64Array` for maximum throughput.
- **TypeScript Ready** — Full type definitions included.

## Installation

```bash
# Bun (Recommended)
bun add dbobj-napi

# NPM
npm install dbobj-napi
```

---

## Module: Database

The `Database` class is the primary interface for all database operations.

### Lifecycle

| Method | Description |
|--------|-------------|
| `new Database(name)` | Create or open a database. `":memory:"` for in-memory, otherwise file-backed (`.dbobj`). |
| `Database.load(path)` | Load a database from a specific file path. |
| `save(path)` | Persist current state to disk. |

### Schema Management

| Method | Description |
|--------|-------------|
| `createTable(name, columns)` | Define a new table. Automatically creates a unique index on any column named `"id"`. |
| `createIndex(table, column)` | Add a standard index for faster lookups. |
| `createUniqueIndex(table, column)` | Add a unique constraint index. |
| `listTables()` | Get all table names. |
| `getTableMetadata(name)` | Get row count and column count for a table. |

### Row Operations (i64)

| Method | Description |
|--------|-------------|
| `insertRowI64(table, values)` | Insert a single row from an array of integers. |
| `insertBatchI64(table, flatValues, numColumns)` | Bulk insert from a flattened `BigInt64Array`. |
| `updateRowI64(table, id, values)` | Update a row by its internal ID. |
| `deleteRow(table, id)` | Delete a row by its internal ID. |
| `getColumnI64(table, column)` | Read an entire column as a zero-copy `BigInt64Array`. |
| `findByI64(table, column, value)` | Find row IDs matching a value. |

### Joins

| Method | Description |
|--------|-------------|
| `hashJoinI64(t1, col1, t2, col2)` | Hash join two tables on matching columns. Returns flat `[id1, id2, ...]` pairs. |

### SQL

| Method | Description |
|--------|-------------|
| `executeSql(sql)` | Execute arbitrary SQL. Returns array of row objects for `SELECT`, or `"OK"` for mutations. |
| `queryI64(sql)` | Execute a `SELECT` and return the first column as zero-copy `BigInt64Array`. |
| `queryJoinI64(sql)` | Execute a `JOIN` query and return all columns as a flattened `BigInt64Array`. |
| `prepare(sql)` | Compile a SQL statement for repeated execution. |

---

## Module: PreparedStatement

Optimized for repeated execution of the same SQL statement with varying parameters.

| Method | Description |
|--------|-------------|
| `run(params)` | Execute once with an array of parameters. |
| `allI64(params)` | Execute a `SELECT` and return results as `BigInt64Array`. |
| `runBatch(batchParams)` | Execute multiple times with a 2D array of parameter sets. |
| `runBatchI64(flatParams, paramsPerRow)` | Execute multiple times from a flattened `BigInt64Array`. 2–3x faster than `runBatch`. |

---

## Module: Types

### `DataType` (enum)

Numeric enum identifying the storage type of a column.

```typescript
export const enum DataType {
  Integer = 0,
  Float = 1,
  String = 2,
  Boolean = 3,
  Blob = 4,
}
```

### `ColumnDefinition` (interface)

Describes a single column when creating a table.

```typescript
export interface ColumnDefinition {
  name: string
  dataType: DataType
  /** Defaults to true if omitted */
  nullable?: boolean
}
```

### `TableMetadata` (interface)

Lightweight table info returned by `getTableMetadata`.

```typescript
export interface TableMetadata {
  name: string
  rowCount: number
  columnCount: number
}
```

---

## Usage Examples

### 1. CRUD Lifecycle

Create a table, insert rows, read a column, update, find, and delete — all with zero-copy reads.

```typescript
import { Database, DataType } from "dbobj-napi";

const db = new Database("CRUD_Test");

// Define schema
db.createTable("users", [
  { name: "age", dataType: DataType.Integer },
]);

// Insert rows
db.insertRowI64("users", [25]);
db.insertRowI64("users", [30]);

// Zero-copy column read — returns BigInt64Array, no JS array overhead
let ages = db.getColumnI64("users", "age");
console.log(ages); // BigInt64Array [ 25n, 30n ]

// Update row by internal ID
db.updateRowI64("users", 0, [35]);

// Find rows matching a value — returns matching IDs
const foundIds = db.findByI64("users", "age", 35);
console.log(foundIds); // BigInt64Array [ 0n ]

// Delete by ID
db.deleteRow("users", 0);
```

### 2. Batch Insert

Insert thousands of rows with a single call using a flattened `BigInt64Array`. Each row is `numColumns` values wide.

```typescript
import { Database, DataType } from "dbobj-napi";

const db = new Database("production");
db.createTable("events", [
  { name: "id", dataType: DataType.Integer },
  { name: "timestamp", dataType: DataType.Integer },
]);

// Flattened: [id1, ts1, id2, ts2, ...]
const batch = new BigInt64Array([
  1n, 1625097600n,
  2n, 1625097660n,
]);
db.insertBatchI64("events", batch, 2);
```

### 3. Hash Join

Server-side hash join returns matching ID pairs as a flat `BigInt64Array`. Avoids creating JS row objects entirely.

```typescript
import { Database, DataType } from "dbobj-napi";

const db = new Database("Join_Test");
db.createTable("t1", [
  { name: "val", dataType: DataType.Integer },
]);
db.createTable("t2", [
  { name: "val", dataType: DataType.Integer },
]);

db.insertRowI64("t1", [10]);
db.insertRowI64("t2", [10]);

// Returns BigInt64Array [ 0n, 0n ] — two IDs that matched
const joinResult = db.hashJoinI64("t1", "val", "t2", "val");
```

### 4. Embedded SQL

Execute arbitrary SQL without needing a separate SQLite or DuckDB process.

```typescript
import { Database } from "dbobj-napi";

const db = new Database("SQL_Test");
db.executeSql("CREATE TABLE users (id INTEGER, name STRING)");
db.executeSql("INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob')");

// SELECT returns an array of row objects
const result = db.executeSql("SELECT * FROM users WHERE id = 1");
console.log(result); // [{ id: 1, name: 'Alice' }]
```

### 5. Prepared Statements

Compile once, execute many times. Use `runBatchI64` for bulk updates with minimal overhead.

```typescript
import { Database } from "dbobj-napi";

const db = new Database("Prep_Test");
db.executeSql("CREATE TABLE users (id INTEGER, val INTEGER)");
db.executeSql("INSERT INTO users (id, val) VALUES (1, 0), (2, 0)");

// Prepare once
const stmt = db.prepare("UPDATE users SET val = ? WHERE id = ?");

// Bulk update via flattened typed array — fastest path
const updates = new BigInt64Array([
  100n, 1n, // val=100 where id=1
  200n, 2n, // val=200 where id=2
]);
stmt.runBatchI64(updates, 2);
```

---

## Benchmarks

Measured against `bun:sqlite` for common workloads:

| Operation | vs bun:sqlite |
|-----------|---------------|
| Column READ (SQL) | ~60x faster |
| Bulk UPDATE (SQL) | ~7x faster |
| Hash JOIN (SQL) | ~4x faster |

Run the local benchmark:

```bash
bun bench.ts
```

## License

MIT / Apache-2.0

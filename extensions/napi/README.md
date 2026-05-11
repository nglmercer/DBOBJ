# DBOBJ N-API Bindings

High-performance native bindings for the **DBOBJ** database engine, designed for **Node.js** and **Bun**.

## Features

- **Zero-Copy Column Access** — Read database columns directly as `BigInt64Array` without serialization overhead or data cloning.
- **Embedded SQL Engine** — Execute SQL queries directly against the database with full `CREATE`, `INSERT`, `SELECT`, `UPDATE`, `DELETE` support.
- **MMap Persistence** — Instant database save/load via memory-mapped files.
- **Hash Joins** — Server-side join execution returns flat ID arrays, avoiding JS object overhead.
- **Batch Operations** — Bulk insert/update via flattened typed arrays for maximum throughput.
- **Typed Methods** — Avoid runtime type dispatch: use `insertRowI64/String/Bool/Float` directly.
- **Batch Update by Column** — `updateBatchI64` updates a single column by ID from a flat typed array.
- **Delete by Column** — `deleteByColumnI64/String/Bool` deletes matching rows and returns count.
- **TypeScript Ready** — Full type definitions included.

## Installation

```bash
# Bun (Recommended)
bun add dbobj-napi

# NPM
npm install dbobj-napi
```

---

## API Reference

### Database — Lifecycle

| Method | Description |
|--------|-------------|
| `new Database(name)` | Create or open a database. `":memory:"` for in-memory, otherwise file-backed (`.dbobj`). |
| `Database.load(path)` | Static — load a database from a specific file path. |
| `save(path)` | Persist current state to disk. |

### Database — Schema

| Method | Description |
|--------|-------------|
| `createTable(name, columns)` | Define a new table. Auto-creates a unique index on any column named `"id"`. |
| `createIndex(table, column)` | Add a standard index for faster lookups. |
| `createUniqueIndex(table, column)` | Add a unique constraint index. |
| `listTables()` | Get all table names. |
| `getTableMetadata(name)` | Get row count and column count for a table. |

### Database — Insert (Single Row)

| Method | Target type | Description |
|--------|-------------|-------------|
| `insertRowI64(table, values)` | `Integer` | Insert from `number[]` |
| `insertRowString(table, values)` | `String` | Insert from `string[]` |
| `insertRowBool(table, values)` | `Boolean` | Insert from `boolean[]` |
| `insertRowFloat(table, values)` | `Float` | Insert from `number[]` |
| `insertRow(table, values)` | Mixed | Insert from `any[]` — auto-detects type per column |

### Database — Insert (Batch)

| Method | Target type | Description |
|--------|-------------|-------------|
| `insertBatchI64(table, flatValues, numCols)` | `Integer` | Bulk insert from flattened `BigInt64Array` |
| `insertBatchString(table, values, numCols)` | `String` | Bulk insert from flattened `string[]` |
| `insertBatchBool(table, values, numCols)` | `Boolean` | Bulk insert from flattened `boolean[]` |
| `insertBatchFloat(table, values, numCols)` | `Float` | Bulk insert from flattened `number[]` |
| `insertBatch(table, values, numCols)` | Mixed | Bulk insert from flattened `any[]` |

### Database — Update / Delete

| Method | Description |
|--------|-------------|
| `updateRowI64(table, id, values)` | Update a row using `number[]` (Integer columns) |
| `updateRowString(table, id, values)` | Update a row using `string[]` (String columns) |
| `updateRowBool(table, id, values)` | Update a row using `boolean[]` (Bool columns) |
| `updateRowFloat(table, id, values)` | Update a row using `number[]` (Float columns) |
| `updateRow(table, id, values)` | Update a row with auto-detected types |
| `updateBatchI64(table, column, values)` | Bulk update a single column by ID from flattened `BigInt64Array` `[newVal, id, ...]` |
| `deleteRow(table, id)` | Delete a row by its internal ID |
| `deleteByColumnI64(table, column, value)` | Delete rows matching an Integer value (returns count) |
| `deleteByColumnString(table, column, value)` | Delete rows matching a String value (returns count) |
| `deleteByColumnBool(table, column, value)` | Delete rows matching a Boolean value (returns count) |

### Database — Read / Find / Meta

| Method | Return type | Description |
|--------|-------------|-------------|
| `getColumnI64(table, column)` | `BigInt64Array` | Read an Integer column — zero-copy |
| `getColumnString(table, column)` | `string[]` | Read a String column |
| `getColumnBool(table, column)` | `boolean[]` | Read a Boolean column |
| `getColumnFloat(table, column)` | `number[]` | Read a Float column |
| `countRows(table)` | `number` | Row count — O(1), no allocation |
| `getRows(table, limit?, offset?)` | `Record<string,any>[]` | Read rows as JSON objects with pagination |
| `findByI64(table, column, value)` | `BigInt64Array` | Find row IDs by Integer value |
| `findByString(table, column, value)` | `BigInt64Array` | Find row IDs by String value |
| `findByBool(table, column, value)` | `BigInt64Array` | Find row IDs by Boolean value |

### Database — Joins

| Method | Description |
|--------|-------------|
| `hashJoinI64(t1, col1, t2, col2)` | Hash join two tables on matching columns. Returns flat `[id1, id2, ...]` pairs. |

### Database — SQL

| Method | Description |
|--------|-------------|
| `executeSql(sql)` | Execute arbitrary SQL. Returns `Record<string,any>[]` for `SELECT`, or `"OK"` for mutations. |
| `queryI64(sql)` | Execute `SELECT` and return first column as zero-copy `BigInt64Array`. |
| `queryJoinI64(sql)` | Execute a `JOIN` query and return columns as a flattened `BigInt64Array`. |
| `prepare(sql)` | Compile a SQL statement for repeated execution. |

---

### PreparedStatement

| Method | Description |
|--------|-------------|
| `run(params)` | Execute once with an array of integer parameters. |
| `allI64(params)` | Execute a `SELECT` and return results as `BigInt64Array`. |
| `runBatch(batchParams)` | Execute multiple times from a 2D array `number[][]`. |
| `runBatchI64(flatParams, paramsPerRow)` | Execute multiple times from a flattened `BigInt64Array`. Fastest path. |
| `runBatchValues(flatParams, paramsPerRow)` | Execute multiple times from a flattened `any[]` (mixed types). |

---

## Types

### `DataType` (enum)

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

```typescript
export interface ColumnDefinition {
  name: string
  dataType: DataType
  /** Defaults to true if omitted */
  nullable?: boolean
}
```

### `TableMetadata` (interface)

```typescript
export interface TableMetadata {
  name: string
  rowCount: number
  columnCount: number
}
```

---

## Usage Examples

### 1. Typed CRUD

Use the exact type method for your column — no runtime type matching.

```typescript
import { Database, DataType } from "dbobj-napi";

const db = new Database("example");

// Define separate tables per type — enables zero-dispatch inserts
db.createTable("users", [
  { name: "id", dataType: DataType.Integer },
  { name: "val", dataType: DataType.Integer },
]);
db.createTable("names", [
  { name: "name", dataType: DataType.String },
]);
db.createTable("flags", [
  { name: "active", dataType: DataType.Boolean },
]);

// Typed inserts — no serde_json::Value, no type dispatch
db.insertBatchI64("users", new BigInt64Array([1n, 100n, 2n, 200n]), 2);
db.insertBatchString("names", ["alice", "bob"], 1);
db.insertBatchBool("flags", [true, false], 1);

// Typed reads
const vals = db.getColumnI64("users", "val"); // BigInt64Array
const names = db.getColumnString("names", "name"); // string[]
const flags = db.getColumnBool("flags", "active"); // boolean[]

// Typed find
const ids = db.findByString("names", "name", "alice"); // BigInt64Array
```

### 2. Mixed-type Insert

For tables with multiple column types, use the generic methods:

```typescript
db.createTable("users", [
  { name: "id", dataType: DataType.Integer },
  { name: "name", dataType: DataType.String },
  { name: "active", dataType: DataType.Boolean },
  { name: "val", dataType: DataType.Integer },
]);

// Single row — types auto-detected
db.insertRow("users", [1, "alice", true, 100]);

// Batch — flat array, all values as JSON-compatible
db.insertBatch("users", [1, "alice", true, 100, 2, "bob", false, 200], 4);

// Read back as JSON
const rows = db.getRows("users");
```

### 3. Batch Insert (Integer-only)

Insert thousands of rows with a single call using a flattened `BigInt64Array`:

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

### 4. Hash Join

Server-side hash join returns matching ID pairs as a flat `BigInt64Array`:

```typescript
const db = new Database("Join_Test");
db.createTable("t1", [{ name: "val", dataType: DataType.Integer }]);
db.createTable("t2", [{ name: "val", dataType: DataType.Integer }]);

db.insertRowI64("t1", [10]);
db.insertRowI64("t2", [10]);

const joinResult = db.hashJoinI64("t1", "val", "t2", "val");
// BigInt64Array [ 0n, 0n ]
```

### 5. Embedded SQL

```typescript
const db = new Database("SQL_Test");
db.executeSql("CREATE TABLE users (id INTEGER, name STRING)");
db.executeSql("INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob')");

const result = db.executeSql("SELECT * FROM users WHERE id = 1");
// [{ id: 1, name: 'Alice' }]
```

### 6. Prepared Statements

Compile once, execute many times:

```typescript
const db = new Database("Prep_Test");
db.executeSql("CREATE TABLE users (id INTEGER, val INTEGER)");
db.executeSql("INSERT INTO users (id, val) VALUES (1, 0), (2, 0)");

const stmt = db.prepare("UPDATE users SET val = ? WHERE id = ?");

// Bulk update via flattened typed array — fastest path
const updates = new BigInt64Array([
  100n, 1n, // val=100 where id=1
  200n, 2n, // val=200 where id=2
]);
stmt.runBatchI64(updates, 2);
```

### 7. Batch Update by Column

Update a single column across many rows using a flat `BigInt64Array`:

```typescript
const db = new Database("BatchUpdate");
db.createTable("t", [{ name: "val", dataType: DataType.Integer }]);
db.insertBatchI64("t", new BigInt64Array([10n, 20n, 30n, 40n]), 1);

// [newVal, id, newVal, id, ...]
db.updateBatchI64("t", "val", new BigInt64Array([99n, 0n, 88n, 2n]));
```

### 8. Delete by Column

Delete all rows matching a value and get the count:

```typescript
const db = new Database("DeleteByCol");
db.createTable("t", [{ name: "val", dataType: DataType.Integer }]);
db.insertRowI64("t", [10]);
db.insertRowI64("t", [20]);
db.insertRowI64("t", [10]);

const deleted = db.deleteByColumnI64("t", "val", 10); // 2
// Only the row with val=20 remains
```

### 9. Row Count

```typescript
const db = new Database("CountRows");
db.createTable("t", [{ name: "x", dataType: DataType.Integer }]);
db.insertRowI64("t", [1]);
db.insertRowI64("t", [2]);
console.log(db.countRows("t")); // 2
```

---

## Benchmarks

Measured against `bun:sqlite` for mixed-type workloads (100K rows):

| Operation | Direct (API) | SQL Bulk | SQL Prep | Bun SQLite |
|-----------|-------------|----------|----------|------------|
| INSERT    | ~100ms      | ~400ms   | ~170ms   | ~550ms     |
| READ      | ~0.6ms      | ~27ms    | ~0.7ms   | ~34ms      |
| FIND      | ~0.02ms     | ~0.08ms  | ~0.07ms  | ~0.25ms    |
| UPDATE    | ~7ms        | ~70ms    | ~4ms     | ~25ms      |
| JOIN      | ~3ms        | ~40ms    | ~5ms     | ~20ms      |

Run the local benchmark:

```bash
bun bench.ts
```

## License

MIT / Apache-2.0

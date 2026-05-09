# DBOBJ N-API Bindings

High-performance native bindings for the **DBOBJ** database engine, designed specifically for **Node.js** and **Bun**.

## 🚀 Key Features

- **Zero-Copy Interop**: Access database columns directly as `BigInt64Array` without memory allocation or data cloning.
- **Embedded SQL Engine**: Fully integrated SQL executor for easy data manipulation.
- **MMap Persistence**: Instant database loading and saving via memory-mapped files.
- **TypeScript Ready**: Full type definitions included for a seamless developer experience.

## 📦 Installation

```bash
# Using Bun (Recommended)
bun add dbobj-napi

# Using NPM
npm install dbobj-napi
```

## 📚 API Reference

### `class Database`

The core class to interact with a DBOBJ database.

#### `constructor(name: string)`
Creates a new database instance or loads an existing one. If `name` is `":memory:"`, creates an in-memory database. Otherwise, creates a file-backed database ending in `.dbobj`.

#### `static load(path: string): Database`
Loads a database directly from the specified file path.

#### `save(path: string): void`
Saves the current database state to the specified file path.

#### `listTables(): Array<string>`
Returns an array of all table names in the database.

#### `createTable(name: string, columnNames: Array<string>, columnTypes: Array<string>): void`
Creates a new table.
- `columnTypes` can be: `"integer"`, `"string"`, `"float"`, `"boolean"`, `"blob"`.

#### `createIndex(tableName: string, columnName: string): void`
Creates a standard index on a specific column to speed up queries.

#### `createUniqueIndex(tableName: string, columnName: string): void`
Creates a unique index on a specific column, ensuring no duplicate values.

#### `getTableMetadata(name: string): TableMetadata | null`
Returns metadata about a table (row count, column count), or `null` if the table does not exist.

#### `insertBatchI64(tableName: string, values: BigInt64Array, numColumns: number): void`
Inserts multiple rows at once using a flattened `BigInt64Array` for extremely high performance.

#### `insertRowI64(tableName: string, values: Array<number>): void`
Inserts a single row of integer values into the table.

#### `updateRowI64(tableName: string, id: number, values: Array<number>): void`
Updates an existing row by its internal ID.

#### `deleteRow(tableName: string, id: number): void`
Deletes a row by its internal ID.

#### `getColumnI64(tableName: string, columnName: string): BigInt64Array`
Retrieves an entire column of 64-bit integers as a zero-copy `BigInt64Array`.

#### `findByI64(tableName: string, columnName: string, value: number): BigInt64Array`
Finds rows matching a specific 64-bit integer value and returns their internal IDs.

#### `hashJoinI64(table1: string, col1: string, table2: string, col2: string): BigInt64Array`
Performs a hash join between two tables on the specified columns. Returns a flat array of matching ID pairs `[id1, id2, ...]`.

#### `executeSql(sql: string): any`
Executes an arbitrary SQL query. Returns an array of objects for `SELECT` queries, or `"OK"` for mutations.

---

### `interface TableMetadata`
```typescript
export interface TableMetadata {
  name: string
  rowCount: number
  columnCount: number
}
```

## 🛠 Usage Examples

### 1. CRUD Operations
```typescript
import { Database } from "dbobj-napi";

const db = new Database("CRUD_Test");
db.createTable("users", ["age"], ["integer"]);

// Insert Data
db.insertRowI64("users", [25]);
db.insertRowI64("users", [30]);

// Zero-Copy Read
let ages = db.getColumnI64("users", "age");
console.log(ages); // BigInt64Array [ 25n, 30n ]

// Update
db.updateRowI64("users", 0, [35]); // Update row ID 0

// Find
const foundIds = db.findByI64("users", "age", 35);
console.log(foundIds); // BigInt64Array [ 0n ]

// Delete
db.deleteRow("users", 0);
```

### 2. High-Performance Batch Inserts
```typescript
import { Database } from "dbobj-napi";

const db = new Database("production");
db.createTable("events", ["id", "timestamp"], ["integer", "integer"]);

// Insert multiple rows instantly using a typed array
const batch = new BigInt64Array([
  1n, 1625097600n, 
  2n, 1625097660n
]);
db.insertBatchI64("events", batch, 2);
```

### 3. Hash Joins
```typescript
import { Database } from "dbobj-napi";

const db = new Database("Join_Test");
db.createTable("t1", ["val"], ["integer"]);
db.createTable("t2", ["val"], ["integer"]);

db.insertRowI64("t1", [10]); // ID 0
db.insertRowI64("t2", [10]); // ID 0

// Get matching pairs of row IDs
const joinResult = db.hashJoinI64("t1", "val", "t2", "val");
// Returns: BigInt64Array [ 0n, 0n ]
```

### 4. SQL Execution
```typescript
import { Database } from "dbobj-napi";

const db = new Database("SQL_Test");

db.executeSql("CREATE TABLE users (id INTEGER, name STRING)");
db.executeSql("INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob')");

const result = db.executeSql("SELECT * FROM users WHERE id = 1");
console.log(result); // [ { id: 1, name: 'Alice' } ]
```

## 📊 Benchmarks

DBOBJ is optimized for read-heavy and analytical workloads in JS environments:
- **Column Reads**: ~30x faster than `bun:sqlite`.
- **Batch Inserts**: ~4x faster than `bun:sqlite`.
- **Point Lookups**: ~8x faster than `bun:sqlite`.

Run the local benchmark:
```bash
bun bench.ts
```

## 📜 License
MIT / Apache-2.0

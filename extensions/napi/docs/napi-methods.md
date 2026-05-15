# NAPI Methods — Full Reference

Complete reference for all public methods on `Database`, `PreparedStatement`, `Schema`,
`DynamicSchema`, `Cursor`, `Transaction`, and `DbError`.

---

## Classes

| Class | Description |
|-------|-------------|
| `Database` | Main entry point — table operations, queries, transactions |
| `PreparedStatement` | Pre-compiled SQL statement for repeated execution |
| `Schema` | Table schema introspection |
| `DynamicSchema` | JSON schema validation and row conversion |
| `Cursor` | Batch iterator for large result sets |
| `Transaction` | ACID transaction handle |
| `DbError` | Structured error from native code |

---

## Enums

### DataType

```typescript
export declare const enum DataType {
  Integer = 0,   // 64-bit signed integer
  Float = 1,     // 64-bit float (f64)
  String = 2,    // UTF-8 string
  Boolean = 3,   // true / false
  Blob = 4,      // Binary data
  Json = 5,      // Arbitrary JSON value
  ArrayString = 6, // string[]
  ArrayI64 = 7,    // BigInt64Array
  ArrayF64 = 8,    // number[] (f64)
}
```

Used as the `dataType` field in `ColumnDefinition` and `SchemaField`.

---

## Interfaces

### ColumnDefinition

```typescript
export interface ColumnDefinition {
  name: string;       // Column name
  dataType: DataType; // Column type
  nullable?: boolean; // Defaults to true if omitted
}
```

### SchemaField

```typescript
export interface SchemaField {
  name: string;
  type: DataType;
  optional?: boolean;
}
```

### TableMetadata

```typescript
export interface TableMetadata {
  name: string;       // Table name
  rowCount: number;   // Number of rows
  columnCount: number; // Number of columns
}
```

---

## `Database`

### constructor

```typescript
new Database(name: string)
```

Create or open a database.

- `name: string` — In-memory name (e.g. `"my_db"`), `":memory:"` for a throwaway DB, or a file path.
  Returns `Database` set up with an auto-generated `.dbobj` extension when a plain name is given.

```typescript
const db = new Database(":memory:");             // in-memory
const db = new Database("my_db");                 // file-backed, saves to my_db.dbobj
const db = new Database("/tmp/my.dbobj");          // explicit path
```

---

### Lifecycle

#### `static load`

```typescript
static load(path: string): Database
```

Load a previously-saved database from disk. Equivalent to `new Database(path)` but
makes the intent explicit.

```typescript
const db = Database.load("/backup/my_data.dbobj");
```

---

#### `save`

```typescript
save(path: string): boolean
```

Persist the entire database to a file using the mmap/rkyv serializer. Returns `true` on
success. Use this for explicit checkpoints when using `new Database(":memory:")`.

```typescript
db.save("/backup/snapshot.dbobj");
```

---

### Schema

#### `createTable`

```typescript
createTable(name: string, columns: ColumnDefinition[]): boolean
```

Define a new table. Returns `true` if the table was created, `false` if it already exists.

| Param | Type | Description |
|-------|------|-------------|
| `name` | `string` | Table name |
| `columns` | `ColumnDefinition[]` | Array of column definitions |

```typescript
db.createTable("users", [
  { name: "id",    dataType: DataType.Integer },
  { name: "name",  dataType: DataType.String  },
  { name: "score", dataType: DataType.Float   },
  { name: "active",dataType: DataType.Boolean },
  { name: "tags",  dataType: DataType.Json    },
]);
```

---

#### `createTableFromSchema`

```typescript
createTableFromSchema(
  tableName: string,
  dynamicSchema: DynamicSchema,
  schemaName: string
): boolean
```

Create a table whose column layout is driven by a registered `DynamicSchema` name.
Useful when the shape of incoming JSON objects is not known at compile time.

| Param | Type | Description |
|-------|------|-------------|
| `tableName` | `string` | Table name |
| `dynamicSchema` | `DynamicSchema` | Schema registry instance |
| `schemaName` | `string` | Key for the registered schema |

```typescript
const ds = new DynamicSchema();
ds.register("User", [
  { name: "id",   type: DataType.Integer },
  { name: "name", type: DataType.String  },
]);
db.createTableFromSchema("users", ds, "User");
```

---

#### `createIndex`

```typescript
createIndex(tableName: string, columnName: string): boolean
```

Create a **non-unique** hash index on `columnName`. Returns `true` on success. Indexes
accelerate `findBy*`, `getRowByColumn*`, and equality-based SQL WHERE lookups from O(N)
to O(1).

```typescript
db.createIndex("users", "email");
```

---

#### `createUniqueIndex`

```typescript
createUniqueIndex(tableName: string, columnName: string): boolean
```

Create a **unique** hash index. Returns `true` on success. Attempting to insert or
replace a duplicate value for this column will fail.

```typescript
db.createUniqueIndex("users", "email");
```

---

#### `createCompositeIndex`

```typescript
createCompositeIndex(tableName: string, columnNames: string[]): boolean
```

Create or refresh indexes for each column in `columnNames`. Returns `true` on success.
This is a convenience method — it does **not** create a multi-column composite index;
call it once per column of interest.

```typescript
db.createCompositeIndex("orders", ["user_id", "status"]);
```

---

#### `listTables`

```typescript
listTables(): string[]
```

Return an array of all table names in the database.

```typescript
const tables = db.listTables(); // ["users", "orders", "products"]
```

---

#### `getTableMetadata`

```typescript
getTableMetadata(name: string): TableMetadata | null
```

Return metadata for a single table, or `null` if the table does not exist.

```typescript
const meta = db.getTableMetadata("users");
console.log(meta.rowCount, meta.columnCount);
```

---

#### `get schema`

```typescript
get schema(): Schema
```

Return a `Schema` instance for introspecting the database without mutating it.

```typescript
const cols = db.schema.getColumnNames("users"); // ["id", "name", "score"]
const type = db.schema.getColumnType("users", "score"); // DataType.Float
```

---

### Insert — Single Row

#### `insertRowI64`

```typescript
insertRowI64(tableName: string, values: number[]): boolean
```

Insert a single row where every column is an integer (`Number` / `int64`). The fastest
insert path — no type dispatch overhead.

| Param | Type | Description |
|-------|------|-------------|
| `tableName` | `string` | Target table |
| `values` | `number[]` | Column values in schema order |

Returns `true` on success, `false` on failure.

---

#### `insertRowString`

```typescript
insertRowString(tableName: string, values: string[]): boolean
```

Insert a single row where every column is a string.

---

#### `insertRowBool`

```typescript
insertRowBool(tableName: string, values: boolean[]): boolean
```

Insert a single row where every column is a boolean.

---

#### `insertRowFloat`

```typescript
insertRowFloat(tableName: string, values: number[]): boolean
```

Insert a single row where every column is a float.

---

#### `insertRow`

```typescript
insertRow(tableName: string, values: any[]): boolean
```

Insert a single row with mixed types. Each value may be a `number`, `string`, `boolean`,
`null`, or `undefined`.

Use `null` / `undefined` for nullable columns.

```typescript
db.insertRow("users", [1, "Alice", 30, true, null]);
```

---

#### `insertOrReplace`

```typescript
insertOrReplace(
  tableName: string,
  values: any[],
  uniqueColumn: string
): boolean
```

Insert a row, or replace the existing row if `uniqueColumn` already contains a matching
value. Useful for upsert / key-value patterns. Requires a unique or primary index on
`uniqueColumn`.

```typescript
db.insertOrReplace("kv", ["email@example.com", '{"name":"Alice"}'], "email");
```

---

#### `insertObject`

```typescript
insertObject(
  tableName: string,
  obj: object,
  dynamicSchema: DynamicSchema,
  schemaName: string
): boolean
```

Insert a single JavaScript object validated and converted against a registered
`DynamicSchema`. The object's properties must match the schema's field names and types
(in a free-form way).

```typescript
const ds = new DynamicSchema();
ds.register("User", [
  { name: "id",   type: DataType.Integer },
  { name: "name", type: DataType.String  },
]);
db.insertObject("users", { id: 1, name: "Alice" }, ds, "User");
```

---

### Insert — Batch

#### `insertBatchI64`

```typescript
insertBatchI64(tableName: string, values: BigInt64Array, numColumns: number): boolean
```

Insert many rows of integer data packed into a single flat `BigInt64Array`. The array
length must be a multiple of `numColumns`.

| Param | Description |
|-------|-------------|
| `tableName` | Target table |
| `values` | Flat `BigInt64Array` — row-major layout: `[r0c0, r0c1, r1c0, r1c1, …]` |
| `numColumns` | Number of columns per row |

```
values = [1, 2, 3, 4]; numColumns = 2  =>  [(1,2), (3,4)]
```

---

#### `insertBatchString`

```typescript
insertBatchString(tableName: string, values: string[], numColumns: number): boolean
```

Same as `insertBatchI64` but for flat `string[]`.

---

#### `insertBatchBool`

```typescript
insertBatchBool(tableName: string, values: boolean[], numColumns: number): boolean
```

Same as `insertBatchI64` but for flat `boolean[]`.

---

#### `insertBatchFloat`

```typescript
insertBatchFloat(tableName: string, values: number[], numColumns: number): boolean
```

Same as `insertBatchI64` but for flat `number[]` (f64).

---

#### `insertBatch`

```typescript
insertBatch(
  tableName: string,
  values: (any | null | undefined)[],
  numColumns: number
): boolean
```

Insert many rows with mixed types packed flat.

```typescript
db.insertBatch(
  "users",
  [1, "Alice", true, 2, "Bob", false],
  3
);
// => [(1,"Alice",true), (2,"Bob",false)]
```

---

#### `insertBatchObjects`

```typescript
insertBatchObjects(
  tableName: string,
  objects: unknown[],
  dynamicSchema: DynamicSchema,
  schemaName: string
): boolean
```

Insert an array of plain JS objects using a registered `DynamicSchema`. Each object is
validated and converted before insertion.

```typescript
db.insertBatchObjects(
  "users",
  [
    { id: 1, name: "Alice" },
    { id: 2, name: "Bob"   },
  ],
  ds,
  "User",
);
```

---

#### `insertBatchColumnar`

```typescript
insertBatchColumnar(tableName: string, columns: object): boolean
```

Insert many rows in **columnar** layout. The `columns` object's keys are column names
and values are arrays of column data.

```typescript
db.insertBatchColumnar({
  id:    [1, 2, 3],
  name:  ["Alice", "Bob", "Carol"],
  score: [99.5, 87.0, 72.1],
});
```

Best for scenarios where your data is already organized by column (e.g. reading from
another columnar source).

---

### Update — Single Row

These methods replace every **non-nullable** column of the row. Provide values for all
non-nullable columns, not just the ones you want to change. For single-column updates,
use the `updateColumn*` methods below instead.

#### `updateRowI64`

```typescript
updateRowI64(tableName: string, id: number, values: number[]): boolean
```

Replace all non-nullable columns of the row identified by `id` using integer values.

---

#### `updateRowString`

```typescript
updateRowString(tableName: string, id: number, values: string[]): boolean
```

Replace all non-nullable columns using string values.

---

#### `updateRowBool`

```typescript
updateRowBool(tableName: string, id: number, values: boolean[]): boolean
```

Replace all non-nullable columns using boolean values.

---

#### `updateRowFloat`

```typescript
updateRowFloat(tableName: string, id: number, values: number[]): boolean
```

Replace all non-nullable columns using float values.

---

#### `updateObject`

```typescript
updateObject(
  tableName: string,
  id: number,
  obj: object,
  dynamicSchema: DynamicSchema,
  schemaName: string
): boolean
```

Update a row by `id` using a plain JS object validated against a `DynamicSchema`.

---

#### `updateRow`

```typescript
updateRow(
  tableName: string,
  id: number,
  values: (any | null | undefined)[]
): boolean
```

Replace all non-nullable columns of the row identified by `id` using mixed types.

```typescript
db.updateRow("users", 1, [1, "Alice Updated", 31]);
```

---

### Update — Single Column

These methods update exactly one column without touching the others. They avoid the
"all non-nullable columns" restriction of `updateRow*`.

#### `updateColumnI64`

```typescript
updateColumnI64(
  tableName: string,
  id: number,
  columnName: string,
  value: number
): boolean
```

Update a single integer column on the row identified by `id`.

---

#### `updateColumnString`

```typescript
updateColumnString(
  tableName: string,
  id: number,
  columnName: string,
  value: string
): boolean
```

Update a single string column.

---

#### `updateColumnBool`

```typescript
updateColumnBool(
  tableName: string,
  id: number,
  columnName: string,
  value: boolean
): boolean
```

Update a single boolean column.

---

#### `updateColumnFloat`

```typescript
updateColumnFloat(
  tableName: string,
  id: number,
  columnName: string,
  value: number
): boolean
```

Update a single float column.

```typescript
db.updateColumnI64("users", 1, "score", 99);
db.updateColumnString("users", 1, "name", "Alice Updated");
```

---

### Update — Batch Column

#### `updateBatchI64`

```typescript
updateBatchI64(
  tableName: string,
  columnName: string,
  values: BigInt64Array
): boolean
```

Bulk-update a single integer column across every matching row simultaneously.
`values` must be a `BigInt64Array` whose length equals the current row count of the
table.

```typescript
const ids = db.getColumnI64("users", "id");
db.updateBatchI64("users", "score", new BigInt64Array([100, 95, 88]));
```

---

### Delete

#### `deleteRow`

```typescript
deleteRow(tableName: string, id: number): boolean
```

Delete the row with the given integer `id`. Returns `true` if a row was removed.

---

#### `deleteBatchI64`

```typescript
deleteBatchI64(tableName: string, ids: BigInt64Array): number
```

Delete multiple rows by their IDs in one call. Returns the number of rows deleted.

```typescript
const ids = db.findByI64("users", "status", 0); // inactive user IDs
db.deleteBatchI64("users", ids);
```

---

#### `deleteByColumnI64`

```typescript
deleteByColumnI64(
  tableName: string,
  columnName: string,
  value: number
): number
```

Delete every row where `columnName` equals the given integer. Returns the number of
rows deleted.

---

#### `deleteByColumnString`

```typescript
deleteByColumnString(
  tableName: string,
  columnName: string,
  value: string
): number
```

Delete every row where `columnName` equals the given string.

---

#### `deleteByColumnBool`

```typescript
deleteByColumnBool(
  tableName: string,
  columnName: string,
  value: boolean
): number
```

Delete every row where `columnName` equals the given boolean.

```typescript
db.deleteByColumnI64("users", "active", 0);
db.deleteByColumnString("users", "status", "banned");
```

---

### Read — All / Paginated Rows

#### `getRows`

```typescript
getRows(
  tableName: string,
  limit?: number | null | undefined,
  offset?: number | null | undefined
): Record<string, any>[]
```

Return all rows (or a page of rows) as an array of plain objects.

| Param | Default | Description |
|-------|---------|-------------|
| `tableName` | — | Table to read |
| `limit` | all | Maximum rows to return |
| `offset` | 0 | Number of rows to skip |

```typescript
const all  = db.getRows("users");
const page = db.getRows("users", 10, 20); // rows 20-29
```

---

#### `getRowsAsync`

```typescript
getRowsAsync(
  tableName: string,
  limit?: number | null | undefined,
  offset?: number | null | undefined
): Promise<Record<string, any>[]>
```

Async version of `getRows`. Suitable for `await` in async contexts. Returns the same
`Record<string,any>[]` shape.

```typescript
const rows = await db.getRowsAsync("users", 100);
```

---

### Read — Single Row Lookup

#### `getRowById`

```typescript
getRowById(tableName: string, id: number): Record<string, any> | null
```

Return the row with the given integer `id`, or `null` if not found.

---

#### `getRowByColumnI64`

```typescript
getRowByColumnI64(
  tableName: string,
  columnName: string,
  value: number
): Record<string, any> | null
```

Return the **first** row where `columnName` equals `value` (integer). Fastest path
when the column is indexed.

---

#### `getRowByColumnString`

```typescript
getRowByColumnString(
  tableName: string,
  columnName: string,
  value: string
): Record<string, any> | null
```

Return the first row where `columnName` equals `value` (string).

---

#### `getRowByColumnBool`

```typescript
getRowByColumnBool(
  tableName: string,
  columnName: string,
  value: boolean
): Record<string, any> | null
```

Return the first row where `columnName` equals `value` (boolean).

```typescript
const user = db.getRowByColumnString("users", "email", "alice@example.com");
const byId = db.getRowById("users", 42);
```

---

### Read — Column Lookup / Aggregates

#### `getColumnI64`

```typescript
getColumnI64(tableName: string, columnName: string): BigInt64Array
```

Return **all** values of an integer column as a zero-copy `BigInt64Array`. No
per-row object allocation.

```typescript
const ids = db.getColumnI64("users", "id");
```

---

#### `getColumnString`

```typescript
getColumnString(tableName: string, columnName: string): string[]
```

Return all values of a string column.

---

#### `getColumnBool`

```typescript
getColumnBool(tableName: string, columnName: string): boolean[]
```

Return all values of a boolean column.

---

#### `getColumnFloat`

```typescript
getColumnFloat(tableName: string, columnName: string): number[]
```

Return all values of a float column.

---

#### `countRows`

```typescript
countRows(tableName: string): number
```

Return the total number of rows in the table. O(1).

---

#### `sumColumn`

```typescript
sumColumn(tableName: string, columnName: string): number
```

Return the sum of all values in `columnName` (numeric columns only).

---

#### `minColumn`

```typescript
minColumn(tableName: string, columnName: string): number
```

Return the smallest value in `columnName`.

---

#### `maxColumn`

```typescript
maxColumn(tableName: string, columnName: string): number
```

Return the largest value in `columnName`.

---

#### `avgColumn`

```typescript
avgColumn(tableName: string, columnName: string): number
```

Return the arithmetic mean of all values in `columnName`. Returns `0` for an empty
table.

```typescript
const total = db.sumColumn("scores", "points");
const high  = db.maxColumn("scores", "points");
const mean  = db.avgColumn("scores", "points");
```

---

### Find — Row ID Lookup by Column Value

These methods return the **row IDs** where `columnName == value`. Returns an empty
`BigInt64Array` if no rows match.

#### `findByI64`

```typescript
findByI64(tableName: string, columnName: string, value: number): BigInt64Array
```

Find row IDs where an integer column equals the given value. O(1) when indexed; falls
back to O(N) scan otherwise.

---

#### `findByString`

```typescript
findByString(tableName: string, columnName: string, value: string): BigInt64Array
```

Find row IDs where a string column equals the given value.

---

#### `findByBool`

```typescript
findByBool(tableName: string, columnName: string, value: boolean): BigInt64Array
```

Find row IDs where a boolean column equals the given value.

```typescript
const bannedIds = db.findByI64("users", "status", 0);
const bobIds    = db.findByString("users", "name", "Bob");
```

---

### Joins

#### `hashJoinI64`

```typescript
hashJoinI64(
  table1: string,
  col1: string,
  table2: string,
  col2: string
): BigInt64Array
```

Perform an **equality hash join** on two integer columns. Returns a flat
`BigInt64Array` of matching row ID pairs: `[t1_row0_id, t2_row0_id, t1_row1_id,
t2_row1_id, …]`.

Use when both join columns are indexed for best performance.

```typescript
const pairs = db.hashJoinI64("orders", "user_id", "users", "id");
// pairs = [0,1, 1,1, 2,2]  => order(0)->user(1), order(1)->user(1), order(2)->user(2)
```

---

### Cursor — Batch Iterator

#### `cursor`

```typescript
cursor(tableName: string, batchSize?: number | null | undefined): Cursor
```

Return a `Cursor` for iterating over large result sets in fixed-size batches without
loading all rows into memory at once.

| Param | Default | Description |
|-------|---------|-------------|
| `tableName` | — | Table to iterate |
| `batchSize` | `5000` | Rows per `next()` call |

```typescript
const cursor = db.cursor("events", 5000);
let batch;
while ((batch = await cursor.next()) !== null) {
  // batch is an array of up to 5000 row objects
  for (const row of batch) {
    // process row
  }
}
```

---

### Transaction

#### `beginTransaction`

```typescript
beginTransaction(): Transaction
```

Begin a transaction. Returns a `Transaction` handle that captures a snapshot of the
current database state. All changes made while the transaction is open can be either
committed or rolled back.

```typescript
const tx = db.beginTransaction();
try {
  db.deleteRow("logs", 42);
  db.insertRow("logs", [Date.now(), "cleanup", true]);
  tx.commit();
} catch (e) {
  tx.rollback();
  throw e;
}
```

---

### SQL

All SQL methods return `DbError` on failure (check `err.message`), or a value described
per method below.

#### `executeSql`

```typescript
executeSql(sql: string): any
```

Execute an arbitrary SQL statement.

| Statement type | Return value |
|---------------|-------------|
| DDL/DML (`CREATE`, `INSERT`, `UPDATE`, `DELETE`, `ALTER`, `DROP`) | `"OK"` (string) |
| `SELECT` | `Record<string, any>[]` — one object per row |
| `SELECT <agg>` | `number[]` — one value per aggregate column |

```typescript
db.executeSql("CREATE TABLE users (id INTEGER, name TEXT)");
db.executeSql("INSERT INTO users VALUES (1, 'Alice')");
const rows = db.executeSql("SELECT * FROM users"); // [{ id:1, name:"Alice" }]
const count = db.executeSql("SELECT COUNT(*) FROM users"); // [1]
```

---

#### `query`

```typescript
query(
  sql: string,
  params?: (any | null | undefined)[]
): PreparedStatement
```

Prepare a parametrised SQL statement. `?` placeholders in `sql` are bound to the values
in `params`. Returns a `PreparedStatement`; call `.get()`, `.all()`, or `.run()` on it.

```typescript
const stmt = db.query("SELECT * FROM users WHERE id = ?", [1]);
const row  = stmt.get(); // { id: 1, name: "Alice" } | null

db.query("UPDATE users SET score = ? WHERE id = ?", [99, 1]).run();
```

---

#### `prepare`

```typescript
prepare(
  sql: string,
  params?: (any | null | undefined)[]
): PreparedStatement
```

Synonym for `query`. Compile a SQL statement for repeated execution without
re-parsing.

```typescript
const insert = db.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
insert.run(["Alice", 30]);
insert.run(["Bob",   25]);
```

---

#### `queryI64`

```typescript
queryI64(sql: string): BigInt64Array
```

Execute a `SELECT` whose first projected column is an integer. Returns the first
column values as a zero-copy `BigInt64Array`.

```typescript
const ids = db.queryI64("SELECT id FROM users WHERE active = 1");
```

---

#### `queryJoinI64`

```typescript
queryJoinI64(sql: string): BigInt64Array
```

Execute a `SELECT` producing a join result and return matching row ID pairs as a flat
`BigInt64Array`. Each pair is `[left_row_id, right_row_id]`.

```typescript
const pairs = db.queryJoinI64(
  "SELECT * FROM orders INNER JOIN users ON orders.user_id = users.id"
);
```

---

### `PreparedStatement`

Returned by `query()` and `prepare()`.

#### `run`

```typescript
run(params?: number[]): boolean
```

Execute a non-SELECT statement (INSERT / UPDATE / DELETE) with `number[]` parameters.
Returns `true` on success.

```typescript
db.query("DELETE FROM users WHERE id = ?").run([42]);
```

---

#### `get`

```typescript
get(params?: (any | null | undefined)[]): Record<string, any> | null
```

Execute a SELECT and return the **first matching row** as a plain object, or `null`
if no rows match.

```typescript
const stmt = db.query("SELECT * FROM users WHERE id = ?", [1]);
const user  = stmt.get(); // { id: 1, name: "Alice", ... } | null
```

---

#### `all`

```typescript
all(params?: (any | null | undefined)[]): Record<string, any>[]
```

Execute a SELECT and return **all matching rows** as an array of plain objects.

```typescript
const rows = db.query("SELECT * FROM users WHERE active = ?", [true]).all();
```

---

#### `allI64`

```typescript
allI64(params?: number[]): BigInt64Array
```

Execute a SELECT and return the **first integer column** values of all matching rows
as a zero-copy `BigInt64Array`.

```typescript
const ids = db.query("SELECT id FROM users").allI64();
```

---

#### `runBatch`

```typescript
runBatch(batchParams: number[][]): boolean
```

Execute a prepared INSERT/UPDATE/DELETE for **multiple parameter sets** — one
`number[]` per invocation. More efficient than calling `.run()` in a loop because the
FFI per-call overhead is amortised.

```typescript
const insert = db.prepare("INSERT INTO scores (val) VALUES (?)");
const rows   = Array.from({ length: 1000 }, (_, i) => [i]);
insert.runBatch(rows);
```

---

#### `runBatchValues`

```typescript
runBatchValues(
  flatParams: (any | null | undefined)[],
  paramsPerRow: number
): boolean
```

Batch form of `run` that takes a **flat** array and the number of parameters per row.
More memory-efficient than `runBatch` (no nested arrays) for large mixed-type batches.

```typescript
const flat = [1, "Alice", 2, "Bob", 3, null]; // paramsPerRow = 2
insert.runBatchValues(flat, 2);
```

---

#### `runBatchI64`

```typescript
runBatchI64(
  flatParams: BigInt64Array,
  paramsPerRow: number
): boolean
```

Batch **integer** form. Prefer this for bulk numeric inserts: it avoids the JS-to-Rust
marshaling cost of `runBatchValues`.

```typescript
const insert = db.prepare("INSERT INTO scores (id, val) VALUES (?, ?)");
const values = new BigInt64Array([1n, 10n, 2n, 20n, 3n, 30n]);
insert.runBatchI64(values, 2); // 2 params per row
```

---

#### `runBatchString`

```typescript
runBatchString(
  flatParams: string[],
  paramsPerRow: number
): boolean
```

Batch form for **all-string** parameter sets.

---

#### `runBatchBool`

```typescript
runBatchBool(
  flatParams: boolean[],
  paramsPerRow: number
): boolean
```

Batch form for **all-boolean** parameter sets.

---

### `Schema`

Returned by `db.schema`.

#### `getColumnNames`

```typescript
getColumnNames(tableName: string): string[]
```

Return an ordered array of all column names in the table.

---

#### `getColumnType`

```typescript
getColumnType(tableName: string, columnName: string): DataType
```

Return the `DataType` of a specific column.

---

#### `hasColumn`

```typescript
hasColumn(tableName: string, columnName: string): boolean
```

Return `true` if `columnName` exists in the table.

---

#### `validateRow`

```typescript
validateRow(tableName: string, values: any[]): string[]
```

Validate a row of values against the table's declared schema. Returns an array of
human-readable violation strings. Returns `[]` (empty) if the row is valid.

```typescript
const errors = db.schema.validateRow("users", [1, "Alice", "not-a-number"]);
// => ["Column 'age' has type INTEGER but received STRING"]
```

---

### `DynamicSchema`

Use when you need flexible JSON-parsing validation before writing to the database.

#### Constructor

```typescript
new DynamicSchema()
```

Create a fresh schema registry.

---

#### `register`

```typescript
register(schemaName: string, fields: SchemaField[]): void
```

Register a named schema for later use with `parse`, `insertObject`, etc.

```typescript
const ds = new DynamicSchema();
ds.register("Order", [
  { name: "id",    type: DataType.Integer },
  { name: "total", type: DataType.Float   },
  { name: "tags",  type: DataType.Json    },
]);
```

---

#### `parse`

```typescript
parse(schemaName: string, buffer: Buffer): any[]
```

Parse a JSON `Buffer` as an array of records, validating each record against the
registered `schemaName`. Uses a streaming tokenizer — invalid JSON or schema mismatches
throw.

```typescript
const records = ds.parse("Order", jsonBuffer);
```

---

#### `parseString`

```typescript
parseString(schemaName: string, input: string): any[]
```

Same as `parse` but accepts a UTF-8 string instead of a `Buffer`.

---

#### `parseOne`

```typescript
parseOne(schemaName: string, buffer: Buffer): any
```

Parse and validate a single JSON record from a `Buffer`.

---

#### `validateObject`

```typescript
validateObject(schemaName: string, obj: object): object
```

Validate a JS object against the registered schema. Returns the same object with no
intermediate conversion. Optional fields that are absent are left absent (not injected
as `null`). Throws on type mismatch or missing required fields.

```typescript
const validated = ds.validateObject("Order", { id: 1n, total: 99.99 });
```

---

#### `toRowValues`

```typescript
toRowValues(schemaName: string, obj: object): (any | null | undefined)[]
```

Convert a validated JS object into an ordered `(any|null|undefined)[]` following the
schema field order. Suitable for passing directly to `db.insertRow()` or
`insertBatch()`.

```typescript
const values = ds.toRowValues("Order", { id: 1n, total: 99.99 });
// => [1n, 99.99, null]
db.insertRow("orders", values);
```

---

#### `validate`

```typescript
validate(schemaName: string, value: any): any
```

Validate a pre-parsed `serde_json::Value` against the schema. Fast path: returns the
original value unmodified when already valid.

---

### `Cursor`

Returned by `db.cursor()`.

#### `next`

```typescript
next(): any | null
```

Advance the cursor by one batch and return the batch (an array of row objects), or
`null` when the result set is exhausted.

```typescript
const cursor = db.cursor("logs", 10000);
let batch = await cursor.next(); // first 10 000 rows
while (batch !== null) {
  for (const row of batch) { /* process */ }
  batch = await cursor.next();
}
```

---

### `Transaction`

Returned by `db.beginTransaction()`.

#### `commit`

```typescript
commit(): boolean
```

Commit all changes made since the transaction was opened. Returns `true` on success.

---

#### `rollback`

```typescript
rollback(): boolean
```

Discard all changes made since the transaction was opened, restoring the database to
its pre-transaction state. Returns `true` on success.

---

### `DbError`

Thrown or returned when native code encounters a fatal error. Not used for SQL syntax
errors (those return `string` error messages instead).

#### `code`

```typescript
get code(): string
```

A stable error code string, e.g. `"TABLE_NOT_FOUND"`, `"TYPE_MISMATCH"`.

---

#### `message`

```typescript
get message(): string
```

A human-readable error description.

```typescript
try {
  db.getRowById("missing_table", 0);
} catch (err) {
  console.log(err.code, err.message);
}
```

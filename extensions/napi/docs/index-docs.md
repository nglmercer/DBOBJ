# TypeScript API Reference

Complete listing of every exported type, interface, and enum from `dbobj-napi`.

---

## Classes

### `Database`

The primary entry point for all database operations. Instances are lightweight and
thread-safe for reads; use `beginTransaction` for multi-step mutations.

| Category | Methods |
|----------|---------|
| Lifecycle | `constructor(name)`, `static load(path)`, `save(path)` |
| Schema | `createTable`, `createTableFromSchema`, `createIndex`, `createUniqueIndex`, `createCompositeIndex`, `listTables`, `getTableMetadata`, `schema` (getter) |
| Insert — single row | `insertRow`, `insertRowI64`, `insertRowString`, `insertRowBool`, `insertRowFloat`, `insertOrReplace`, `insertObject` |
| Insert — batch | `insertBatch`, `insertBatchI64`, `insertBatchString`, `insertBatchBool`, `insertBatchFloat`, `insertBatchObjects`, `insertBatchColumnar` |
| Update | `updateRow`, `updateRowI64`, `updateRowString`, `updateRowBool`, `updateRowFloat`, `updateObject`, `updateColumnI64`, `updateColumnString`, `updateColumnBool`, `updateColumnFloat`, `updateBatchI64` |
| Delete | `deleteRow`, `deleteBatchI64`, `deleteByColumnI64`, `deleteByColumnString`, `deleteByColumnBool` |
| Read | `getRows`, `getRowsAsync`, `getRowById`, `getRowByColumnI64`, `getRowByColumnString`, `getRowByColumnBool` |
| Column / aggregate | `getColumnI64`, `getColumnString`, `getColumnBool`, `getColumnFloat`, `countRows`, `sumColumn`, `minColumn`, `maxColumn`, `avgColumn` |
| Find | `findByI64`, `findByString`, `findByBool` |
| Joins | `hashJoinI64` |
| Cursor | `cursor` |
| Transaction | `beginTransaction` |
| SQL | `executeSql`, `query`, `prepare`, `queryI64`, `queryJoinI64` |

---

### `PreparedStatement`

Returned by `db.query(sql, params?)` and `db.prepare(sql, params?)`.

| Method | Description |
|--------|-------------|
| `run(params?)` | Execute INSERT / UPDATE / DELETE; returns `boolean` |
| `get(params?)` | Execute SELECT; return first row object or `null` |
| `all(params?)` | Execute SELECT; return all matching rows as `Record<string,any>[]` |
| `allI64(params?)` | Execute SELECT; return first column as `BigInt64Array` |
| `runBatch(batchParams)` | Execute multiple `number[]` parameter sets at once |
| `runBatchValues(flatParams, paramsPerRow)` | Execute with flat mixed-type array |
| `runBatchI64(flatParams, paramsPerRow)` | Execute with flat `BigInt64Array` |
| `runBatchString(flatParams, paramsPerRow)` | Execute with flat `string[]` |
| `runBatchBool(flatParams, paramsPerRow)` | Execute with flat `boolean[]` |

---

### `Schema`

Returned by `db.schema`. Provides introspection without mutation.

| Method | Return | Description |
|--------|--------|-------------|
| `getColumnNames(tableName)` | `string[]` | All column names in order |
| `getColumnType(tableName, columnName)` | `DataType` | Type enum of a column |
| `hasColumn(tableName, columnName)` | `boolean` | Whether column exists |
| `validateRow(tableName, values)` | `string[]` | Schema violations (empty = valid) |

---

### `DynamicSchema`

JSON schema registry that validates and converts plain objects into row values.

#### Constructor

```typescript
new DynamicSchema()
```

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `register` | `(schemaName: string, fields: SchemaField[]): void` | Register a named schema |
| `parse` | `(schemaName: string, buffer: Buffer): any[]` | Parse + validate JSON Buffer as array of records |
| `parseString` | `(schemaName: string, input: string): any[]` | Same as `parse` from UTF-8 string |
| `parseOne` | `(schemaName: string, buffer: Buffer): any` | Parse + validate a single record |
| `validateObject` | `(schemaName: string, obj: object): object` | Validate JS object, return it directly |
| `toRowValues` | `(schemaName: string, obj: object): any[]` | Convert valid object to ordered row values |
| `validate` | `(schemaName: string, value: any): any` | Validate a pre-parsed serde_json::Value |

---

### `Cursor`

Returned by `db.cursor(tableName, batchSize?)`.

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `next` | `(): Promise<any \| null>` | Advance one batch; returns `Record<string,any>[]` or `null` |

---

### `Transaction`

Returned by `db.beginTransaction()`. Captures a snapshot of the DB at the time of
creation.

#### Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `commit` | `(): boolean` | Accept all changes since the snapshot |
| `rollback` | `(): boolean` | Discard all changes, restore snapshot |

---

### `DbError`

Structured error object returned by native code on fatal failures (table not found,
type mismatch, etc.). Not used for SQL syntax errors.

#### Properties

| Property | Type | Description |
|----------|------|-------------|
| `.code` | `string` | Stable machine-readable error code |
| `.message` | `string` | Human-readable description |

---

## Interfaces

### `ColumnDefinition`

```typescript
interface ColumnDefinition {
  name: string;           // Column name
  dataType: DataType;     // Column type (required)
  nullable?: boolean;     // Defaults to true if omitted
}
```

### `SchemaField`

```typescript
interface SchemaField {
  name: string;           // Field name
  type: DataType;         // Field type (required)
  optional?: boolean;     // When true, absent fields are not injected as null
}
```

### `TableMetadata`

```typescript
interface TableMetadata {
  name: string;           // Table name
  rowCount: number;       // Number of rows
  columnCount: number;    // Number of columns
}
```

---

## Enums

### `DataType`

```typescript
const enum DataType {
  Integer     = 0,   // 64-bit signed integer  (i64)
  Float       = 1,   // 64-bit float           (f64)
  String      = 2,   // UTF-8 string
  Boolean     = 3,   // true / false
  Blob        = 4,   // Binary data
  Json        = 5,   // Arbitrary JSON
  ArrayString = 6,   // string[]
  ArrayI64    = 7,   // BigInt64Array (i64[])
  ArrayF64    = 8,   // number[] (f64[])
}
```

Use `DataType` as the `dataType` field in `ColumnDefinition` and the `type` field in
`SchemaField`.

---

## Zero-Copy Types

### `BigInt64Array`

Identical to the native JavaScript `BigInt64Array`. Used for:

- Column reads (`getColumnI64`)
- Flat typed batch inserts (`insertBatchI64`, `runBatchI64`)
- Batch ID results (`findByI64`, `hashJoinI64`, `queryI64`, `allI64`)

Data is shared via `SharedArrayBuffer` under the hood — reading a column is a single
pointer read with zero cloning.

### `Buffer` (Node.js)

The `Buffer` type used by `DynamicSchema.parse(schemaName, buffer: Buffer)`. Pass any
Node.js `Buffer` (including an `ArrayBuffer` view).

---

## Performance Tier Cheat Sheet

| Speed | API tier |
|-------|----------|
| Fastest | Typed single-row / typed batch (`insertRowI64`, `insertBatchI64`, `getColumnI64`, `updateColumnI64`) |
| Fast | Typed batch run (`runBatchI64`) |
| Medium | SQL with `?` placeholders |
| Slowest | `insertRow`(mixed) / `executeSql` without parameters |

Typed methods avoid the JavaScript → Rust value-marshaling overhead entirely.

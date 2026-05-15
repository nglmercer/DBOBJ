# NAPI Methods

## Database — Lifecycle

| Method | Description |
|--------|-------------|
| `new Database(name)` | Create or open a database |
| `Database.load(path)` | Load from file |
| `save(path)` | Persist to disk |

## Database — Schema

| Method | Description |
|--------|-------------|
| `createTable(name, columns)` | Define a table |
| `createIndex(table, column)` | Add index |
| `createUniqueIndex(table, column)` | Add unique index |
| `createCompositeIndex(table, columns)` | Index multiple columns |
| `listTables()` | List table names |
| `getTableMetadata(name)` | Row/column count |
| `get schema()` | Schema introspection |

## Database — Insert

### Single Row

| Method | Type | Description |
|--------|------|-------------|
| `insertRowI64(table, values)` | Integer | `number[]` |
| `insertRowString(table, values)` | String | `string[]` |
| `insertRowBool(table, values)` | Boolean | `boolean[]` |
| `insertRowFloat(table, values)` | Float | `number[]` |
| `insertRow(table, values)` | Mixed | `any[]` |

### Batch

| Method | Type | Description |
|--------|------|-------------|
| `insertBatchI64(table, values, numCols)` | Integer | Flat `BigInt64Array` |
| `insertBatchString(table, values, numCols)` | String | Flat `string[]` |
| `insertBatchBool(table, values, numCols)` | Boolean | Flat `boolean[]` |
| `insertBatchFloat(table, values, numCols)` | Float | Flat `number[]` |
| `insertBatch(table, values, numCols)` | Mixed | Flat `any[]` |

### Upsert

| Method | Description |
|--------|-------------|
| `insertOrReplace(table, values, uniqueColumn)` | Insert or replace |

## Database — Update / Delete

| Method | Description |
|--------|-------------|
| `updateRowI64/String/Bool/Float(table, id, values)` | Update row by ID |
| `updateRow(table, id, values)` | Update with auto-detected types |
| `updateColumnI64/String/Bool/Float(table, id, column, value)` | Update single column |
| `updateBatchI64(table, column, values)` | Bulk update single column |
| `deleteRow(table, id)` | Delete by ID |
| `deleteBatchI64(table, ids)` | Delete multiple by ID |
| `deleteByColumnI64/String/Bool(table, column, value)` | Delete matching rows |

## Database — Read

| Method | Return Type | Description |
|--------|-------------|-------------|
| `getColumnI64(table, column)` | `BigInt64Array` | Integer column — zero-copy |
| `getColumnString(table, column)` | `string[]` | String column |
| `getColumnBool(table, column)` | `boolean[]` | Boolean column |
| `getColumnFloat(table, column)` | `number[]` | Float column |
| `getRows(table, limit?, offset?)` | `Record<string,any>[]` | Rows as JSON |
| `getRowById(table, id)` | `Record<string,any> | null` | Single row by ID |
| `getRowByColumnI64/String/Bool(table, column, value)` | `Record<string,any> | null` | First matching row |
| `countRows(table)` | `number` | Row count — O(1) |
| `sumColumn/minColumn/maxColumn(table, column)` | `number` | Aggregation |
| `avgColumn(table, column)` | `number` | Average |

## Database — Find

| Method | Return Type | Description |
|--------|-------------|-------------|
| `findByI64(table, column, value)` | `BigInt64Array` | Match IDs by Integer |
| `findByString(table, column, value)` | `BigInt64Array` | Match IDs by String |
| `findByBool(table, column, value)` | `BigInt64Array` | Match IDs by Boolean |

## Database — Joins

| Method | Description |
|--------|-------------|
| `hashJoinI64(t1, col1, t2, col2)` | Hash join, returns ID pairs |

## Database — SQL

| Method | Description |
|--------|-------------|
| `executeSql(sql)` | Execute SQL, returns rows or `"OK"` |
| `query(sql, params?)` | Prepare statement with optional params |
| `queryI64(sql)` | SELECT first column as `BigInt64Array` |
| `queryJoinI64(sql)` | JOIN query as flat array |
| `prepare(sql, params?)` | Compile statement for reuse |

## Database — Transactions

| Method | Description |
|--------|-------------|
| `beginTransaction()` | Snapshot current state |
| `tx.commit()` | Accept changes |
| `tx.rollback()` | Restore snapshot |

## Database — Async

| Method | Description |
|--------|-------------|
| `getRowsAsync(table, limit?, offset?)` | `Promise<any>` — non-blocking read |
| `cursor(table, batchSize?)` | `Cursor` — batch iterator |

## PreparedStatement

| Method | Description |
|--------|-------------|
| `run(params?)` | Execute with `number[]` |
| `all(params?)` | SELECT as `Record<string,any>[]` |
| `get(params?)` | SELECT first row as `Record<string,any>` |
| `allI64(params?)` | SELECT as `BigInt64Array` |
| `runBatch(params)` | 2D `number[][]` batch |
| `runBatchI64(values, paramsPerRow)` | Flat `BigInt64Array` batch |
| `runBatchValues(values, paramsPerRow)` | Flat `any[]` batch |

## Schema Introspection

| Method | Return | Description |
|--------|--------|-------------|
| `schema.getColumnNames(table)` | `string[]` | Column names |
| `schema.getColumnType(table, column)` | `DataType` | Column type |
| `schema.hasColumn(table, column)` | `boolean` | Column exists |
| `schema.validateRow(table, values)` | `string[]` | Validation errors |

## Cursor

| Method | Description |
|--------|-------------|
| `cursor.next()` | Returns batch or `null` |

## Transaction

| Method | Description |
|--------|-------------|
| `tx.commit()` | Accept changes |
| `tx.rollback()` | Revert to snapshot |

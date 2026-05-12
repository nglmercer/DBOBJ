# Architecture

## Storage Layer

DBOBJ uses a **flat columnar storage** layout:

```
data: Vec<Value>
[row0_col0, row0_col1, row1_col0, row1_col1, ...]
```

Each row is stored contiguously for cache locality. Accessing row `i` column `j` is `data[i * num_columns + j]` — O(1).

### Persistence

- **mmap + rkyv**: Zero-copy serialization. The entire database is saved as a memory-mapped file.
- **WAL** (optional): Write-ahead log for crash recovery.

## Indexing

- **Sequential IDs**: When no custom `id` column exists, rows get sequential `Id::Integer(0), Id::Integer(1), ...`.
- **Hash indexes**: Single-column indexes use `FastHashMap<Value, Vec<Id>>` for non-unique and `FastHashMap<Value, usize>` for unique.
- **`get_index`**: O(1) index lookup for sequential IDs, O(1) hash lookup for `id_map`.

## SQL Engine

The SQL engine uses a hand-written **recursive descent parser** with precedence climbing:

```
parse_expr → parse_or → parse_and → parse_like → parse_comparison → parse_atom
```

The executor maps SQL AST to core expressions and evaluates them via `table.select(|row| predicate(row))`.

## NAPI Bridge

JavaScript values cross the boundary as:
- **`BigInt64Array`**: Zero-copy integer columns (shared memory via `SharedArrayBuffer`)
- **`Vec<String>`**: String columns as JS arrays
- **`Vec<bool>`**: Boolean columns as JS arrays
- **`serde_json::Value`**: Mixed-type rows as JSON objects (for `getRows`, `insertRow`)

## Performance Design

- **Batch operations** avoid per-row overhead by working on flat typed arrays
- **Column reads** return `BigInt64Array` which is zero-copy (pointer sharing, no cloning)
- **Typed methods** (`insertRowI64`, `insertBatchString`) skip runtime type dispatch
- **Cursor** batches rows for memory-efficient iteration over large datasets

# DBOBJ: High-Performance Modular Database Engine

DBOBJ is a high-performance, in-memory database engine written in Rust, designed for extreme speed, low latency, and zero-copy data access. It is modularized into three core components:

1.  **Core Engine (`dbobj`)**: The foundation. High-performance columnar/dense-row storage with `mmap` and `rkyv` support.
2.  **SQL Extension (`dbobj-sql`)**: A high-performance SQL parser and executor specialized for DBOBJ.
3.  **N-API Bridge (`dbobj-napi`)**: Native bindings for **Node.js** and **Bun**, providing zero-copy access to the Rust engine.

---

## 📦 Project Structure

### 1. Core Engine (`/`)
The core library provides the fundamental database primitives:
- **Dense Row Storage**: Optimized for cache locality.
- **MMap + rkyv**: Instant database loading with zero-copy deserialization.
- **Shared Memory**: Thread-safe access via `Arc<RwLock<...>>`.

### 2. SQL Extension (`extensions/sql`)
A specialized SQL implementation that is **up to 60x faster** than generic SQLite drivers.
- **LocalParser**: Hand-written recursive descent parser.
- **Optimized Executor**: Directly targets the core columnar storage.

### 3. N-API Bindings (`extensions/napi`)
The high-performance bridge for the JavaScript ecosystem.
- **Bun/Node Support**: Pre-built binaries for high-speed integration.
- **Zero-Copy Buffers**: Export entire columns as `BigInt64Array` without cloning data.

---

## Features

### Storage
- **Columnar in-memory layout** — each row is stored contiguously for cache locality (`O(1)` access).
- **mmap + rkyv persistence** — zero-copy serialisation; load a 10 GB database in microseconds.
- **WAL** (optional) — write-ahead log for crash recovery.

### Indexing
- **Sequential IDs** — rows are addressed by auto-incrementing integer IDs.
- **Hash indexes** — `FastHashMap<Value, Vec<Id>>` for non-unique, `FastHashMap<Value, usize>` for unique.
- **Composite indexes** — index multiple columns with one call.
- **Find by column value** — `findByI64/`findByString`/`findByBool` return matching row IDs.

### Query Engine
- **Hand-written recursive-descent SQL parser** with precedence climbing.
- **Equality hash join** — `hashJoinI64` returns matched row ID pairs in O(N+M).
- **Filter, sort and paginate** entirely through the SQL engine.
- **`?` parameter binding** — prevent SQL injection without string interpolation.

### Ingestion
- **Typed batch inserts** — `insertBatchI64`, `insertBatchString`, `insertBatchFloat`, `insertBatchBool` flat arrays avoid any type-dispatch overhead.
- **Mixed-type batch** — `insertBatch` packs `any[]` values into one FFI call.
- **Columnar batch** — `insertBatchColumnar` takes `{ columnName: values[] }`.
- **DynamicSchema** — validate and convert JSON objects before writing to a known-schema table.

### API
- **`getColumnI64`** — zero-copy column reads as `BigInt64Array` (pointer overhead only).
- **`cursor`** — batch-iterate over multi-million-row tables without materialising the entire result set.
- **`beginTransaction`** — snapshot-based commit/rollback.
- **Async API** — `getRowsAsync` and `cursor.next()` return Promises for seamless integration with `async`/`await`.

---

## 🛠 Usage (Bun / Node.js)

```typescript
import { Database, DataType } from "dbobj-napi";

// Open or create a database
const db = new Database("my_db");

// Define a table schema
db.createTable("users", [
  { name: "id",       dataType: DataType.Integer  },
  { name: "name",     dataType: DataType.String   },
  { name: "age",      dataType: DataType.Integer  },
  { name: "active",   dataType: DataType.Boolean  },
  { name: "score",    dataType: DataType.Float    },
]);

// Insert rows
db.insertRow("users", [1, "Alice",    30, true,  99.5]);
db.insertRow("users", [2, "Bob",      25, true,  87.3]);
db.insertRow("users", [3, "Carol",    28, false, null]);

// Read
const rows = db.getRows("users");

// Or use SQL for complex queries
const adults = db.executeSql(
  "SELECT * FROM users WHERE age > ? AND active = ? ORDER BY score DESC",
  18, true
);

// Or use a prepared statement for repeated execution
const stmt = db.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
stmt.run(["Dave",    40]);
stmt.run(["Eve",     22]);
```

---

## 📚 Documentation

- [Getting Started](./extensions/napi/docs/getting-started.md) — install and quickstart
- [NAPI Methods](./extensions/napi/docs/napi-methods.md) — full method reference
- [SQL Reference](./extensions/napi/docs/sql-reference.md) — supported SQL syntax
- [Examples](./extensions/napi/docs/examples.md) — usage examples
- [Benchmarks](./extensions/napi/docs/benchmarks.md) — performance numbers
- [Architecture](./extensions/napi/docs/architecture.md) — engine design

---

## 📈 Benchmarking Methodology

We maintain three benchmark suites:
1.  **Rust Core**: `cargo bench` (Criterion) for micro-benchmarks of the engine.
2.  **SQL Parser**: `cargo bench -p dbobj-sql` to compare parsing overhead.
3.  **End-to-End**: `bun bench.ts` in the `extensions/napi` directory for a full real-world comparison against `bun:sqlite`.

### Running the End-to-End Bench
```bash
cd extensions/napi
npm run build
bun bench/index.ts
```

---

## 📜 License
MIT / Apache-2.0

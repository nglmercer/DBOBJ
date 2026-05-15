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

## 🛠 Usage (Bun / Node.js)

```typescript
import { Database, DataType } from "dbobj-napi";

const db = new Database(":memory:");

// Create a table
db.createTable("users", [
  { name: "id", dataType: DataType.Integer },
  { name: "name", dataType: DataType.String },
  { name: "active", dataType: DataType.Boolean },
]);

// Insert rows
db.insertRow("users", [1, "Alice", true]);
db.insertRow("users", [2, "Bob", false]);

// Read as JSON
const rows = db.getRows("users");
console.log(rows);
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
bun bench.ts
```

---

## 📜 License
MIT / Apache-2.0

# DBOBJ: High-Performance Modular Database Engine

DBOBJ is a high-performance, in-memory database engine written in Rust, designed for extreme speed, low latency, and zero-copy data access. It is modularized into three core components:

1.  **Core Engine (`dbobj`)**: The foundation. High-performance columnar/dense-row storage with `mmap` and `rkyv` support.
2.  **SQL Extension (`dbobj-sql`)**: A high-performance SQL parser and executor specialized for DBOBJ.
3.  **N-API Bridge (`dbobj-napi`)**: Native bindings for **Node.js** and **Bun**, providing zero-copy access to the Rust engine.

---

## 🚀 Performance Snapshot (N-API / Bun)

Results comparing **DBOBJ (Native API)**, **DBOBJ (SQL Engine)**, and **Bun SQLite** on 100,000 rows:

| Operation | DBOBJ Direct | **DBOBJ SQL (Prep)** | Bun SQLite | Speedup (vs SQLite) |
| :--- | :--- | :--- | :--- | :--- |
| **INSERT (Batch)** | 43.72 ms | **57.88 ms** | 168.25 ms | **2.9x** |
| **READ (Column)** | 0.46 ms | **0.68 ms** | 24.36 ms | **35.8x** |
| **FIND (Indexed)** | 0.01 ms | **0.05 ms** | 0.15 ms | **3.0x** |
| **UPDATE (Bulk)** | 5.96 ms | **3.02 ms** | 14.43 ms | **4.7x** |
| **JOIN (Hash)** | 4.14 ms | 36.09 ms | 18.46 ms | 0.5x |

> *Note: DBOBJ Direct uses N-API with `SharedArrayBuffer` / `BigInt64Array` for zero-copy transfers, bypassing the serialization overhead typical of JS-to-Native bridges.*

---

## 📦 Project Structure

### 1. Core Engine (`/`)
The core library provides the fundamental database primitives:
- **Dense Row Storage**: Optimized for cache locality.
- **MMap + rkyv**: Instant database loading with zero-copy deserialization.
- **Shared Memory**: Thread-safe access via `Arc<RwLock<...>>`.

### 2. SQL Extension (`extensions/sql`)
A specialized SQL implementation that is **up to 35x faster** than generic SQLite drivers.
- **LocalParser**: Hand-written recursive descent parser.
- **Optimized Executor**: Directly targets the core columnar storage.

### 3. N-API Bindings (`extensions/napi`)
The high-performance bridge for the JavaScript ecosystem.
- **Bun/Node Support**: Pre-built binaries for high-speed integration.
- **Zero-Copy Buffers**: Export entire columns as `BigInt64Array` without cloning data.

---

## 🛠 Usage (Bun / Node.js)

```typescript
import { Database } from "dbobj-napi";

const db = new Database("my_db");

// 1. Direct API (Maximum Performance)
db.createTable("users", ["id", "val"], ["integer", "integer"]);
db.createIndex("users", "id");

// 2. SQL API (Ease of Use)
db.executeSql("INSERT INTO users (id, val) VALUES (1, 100)");
const results = db.executeSql("SELECT * FROM users WHERE id = 1");

// 3. Zero-Copy Column Access
const columnData = db.getColumnI64("users", "val"); // BigInt64Array
```

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

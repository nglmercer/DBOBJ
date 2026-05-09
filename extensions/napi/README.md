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

## 🛠 Usage

```typescript
import { Database } from "dbobj-napi";

const db = new Database("production");

// Create table and indices
db.createTable("events", ["id", "timestamp"], ["integer", "integer"]);
db.createUniqueIndex("events", "id");

// Batch insert (High Performance)
const batch = new BigInt64Array([1n, 1625097600n, 2n, 1625097660n]);
db.insertBatchI64("events", batch, 2);

// SQL Query
const logs = db.executeSql("SELECT * FROM events WHERE timestamp > 1625097600");

// Zero-Copy Read
const timestamps = db.getColumnI64("events", "timestamp");
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

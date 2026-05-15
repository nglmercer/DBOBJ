# DBOBJ

[![npm version](https://img.shields.io/npm/v/dbobj-napi.svg)](https://www.npmjs.com/package/dbobj-napi)
[![Bun](https://img.shields.io/badge/bun-supported-green)](https://bun.sh)
[![License: MIT](https://img.shields.io/badge/License-Apache--2.0-blue)](https://github.com/nicobailon/dbobj)

High-performance modular database engine for Rust, Node.js, and Bun.

| Component | Description |
|-----------|-------------|
| **Core** (`dbobj`) | Columnar storage, mmap persistence, hash joins |
| **SQL** (`dbobj-sql`) | Embedded SQL parser and executor |
| **NAPI** (`dbobj-napi`) | Native Node.js/Bun bindings |

---

## Install

```bash
bun add dbobj-napi
# or
npm install dbobj-napi
```

No native compilation step is needed — prebuilt binaries are provided for Linux x64,
macOS x64/arm64, and Windows x64 for Node.js 18+ and Bun 1.0+.

---

## Quick Start

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

## Database Lifecycle

```typescript
// In-memory — fastest, data gone on exit
const db = new Database(":memory:");

// File-backed — auto-saves to ./my_db.dbobj
const db = new Database("my_db");
```

---

## SQL Support

```sql
CREATE TABLE  users (
  id    INTEGER NOT NULL,
  name  TEXT    DEFAULT 'guest',
  email TEXT    NOT NULL
);

INSERT INTO users (id, name, email)
  VALUES (1, 'Alice', 'alice@example.com'),
         (2, 'Bob',   'bob@example.com');

SELECT * FROM users WHERE age > 18 ORDER BY score DESC LIMIT 10;
SELECT COUNT(*), AVG(score), MAX(age) FROM users WHERE active = true;

UPDATE users SET active = true WHERE id = 1;
DELETE FROM users WHERE id = 2;

DROP TABLE IF EXISTS users;
```

Full SQL reference: [docs/sql-reference.md](./docs/sql-reference.md)

---

## TypeScript Types

Every public symbol is typed. Full reference: [docs/index-docs.md](./docs/index-docs.md)

```typescript
import {
  Database,
  DataType,       // Integer | Float | String | Boolean | Blob | Json | Array* |
  ColumnDefinition,
  SchemaField,
  TableMetadata,

  PreparedStatement,
  Cursor,
  Schema,
  DynamicSchema,
  Transaction,
  DbError,
} from "dbobj-napi";
```

---

## Examples

39 examples covering every API area: [docs/examples.md](./docs/examples.md)

Key patterns:

```typescript
// 1. Key-value store
db.createUniqueIndex("kv", "key");
db.insertOrReplace("kv", [k, v], "key");
db.getRowByColumnString("kv", "key", k)?.value ?? null;

// 2. Batch processing — stream large tables
const cursor = db.cursor("events", 5000);
let batch;
while ((batch = await cursor.next()) !== null) {
  for (const row of batch) process(row);
}

// 3. Transactional updates
const tx = db.beginTransaction();
try { update(); tx.commit(); } catch { tx.rollback(); }

// 4. Zero-copy column reads
const ids: BigInt64Array = db.getColumnI64("users", "id");

// 5. Hash join on integer columns
const pairs = db.hashJoinI64("orders", "user_id", "users", "id");

// 6. Gas-on-production Ingestion
db.insertBatchFloat("users", installFlatArray, numCols); // no type dispatch
```

---

## Benchmarks

| Operation | DBOBJ | SQLite JS | Speedup |
|:----------|------:|----------:|--------:|
| Insert (single) | ~1.3 M ops/s | ~265 K ops/s | 5.0× |
| Insert (batch 100) | ~2.9 M ops/s | ~1.6 M ops/s | 1.8× |
| Read (by ID) | ~5.7 M ops/s | ~275 K ops/s | 20.6× |
| Search (indexed) | ~4.0 M ops/s | ~226 K ops/s | 17.8× |
| Hash join (100K row) | 30.6 /s | 10.1 /s | 3.0× |

Full benchmark suite: [docs/benchmarks.md](./docs/benchmarks.md)

Run locally:
```bash
cd extensions/napi
bun run build && bun bench.ts
```

---

## Documentation

- [Getting Started](extensions/napi/docs/getting-started.md) — installation + 10-line quickstart
- [TypeScript Types](extensions/napi/docs/index-docs.md) — full enum / interface / class reference
- [NAPI Methods](extensions/napi/docs/napi-methods.md) — every method with params, return types, and examples
- [SQL Reference](extensions/napi/docs/sql-reference.md) — grammar, operators, aggregates, error handling
- [Architecture](extensions/napi/docs/architecture.md) — storage, indexes, parser, NAPI bridge
- [Performance](extensions/napi/docs/benchmarks.md) — Criterion and end-to-end benchmarks
- [Usage Examples](extensions/napi/docs/examples.md) — 39 runnable examples for every API area

---

## License

Dual-licensed under either of

- [Apache License, Version 2.0](https://opensource.org/license/apache-2-0/)
- [MIT License](https://opensource.org/license/mit/)

at your option.

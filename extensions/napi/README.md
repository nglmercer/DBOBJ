# DBOBJ

High-performance modular database engine for Rust, Node.js, and Bun.

| Component | Description |
|-----------|-------------|
| **Core** (`dbobj`) | Columnar storage, mmap persistence, hash joins |
| **SQL** (`dbobj-sql`) | Embedded SQL parser and executor |
| **NAPI** (`dbobj-napi`) | Native Node.js/Bun bindings |

## Docs

- [Getting Started](./docs/getting-started.md) — installation and quickstart
- [API Reference](./docs/api-reference.md) — full method reference
- [NAPI Methods](./docs/napi-methods.md) — all native methods with types
- [SQL Reference](./docs/sql-reference.md) — supported SQL syntax
- [Architecture](./docs/architecture.md) — engine design overview
- [Benchmarks](./docs/benchmarks.md) — performance numbers
- [Examples](./docs/examples.md) — usage examples

## Quick Install

```bash
bun add dbobj-napi
# or
npm install dbobj-napi
```

```typescript
import { Database, DataType } from "dbobj-napi";
const db = new Database("my_db");

// High-performance SQL with bound parameters
const stmt = db.query("SELECT * FROM users WHERE id = ?", [1]);
const user = stmt.get();
```

## Performance

DBOBJ provides multiple ingestion and query strategies. Below are results for 100K rows (see [Full Benchmarks](./docs/benchmarks.md) for details).

## License

MIT / Apache-2.0

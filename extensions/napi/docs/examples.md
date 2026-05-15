# Examples

Real-world usage patterns covering every API area.

---

## 1 — Getting Started

```typescript
import { Database, DataType } from "dbobj-napi";

// In-memory (fastest, data lost on exit)
const db = new Database(":memory:");

// File-backed — saves to ./my_db.dbobj automatically
const db = new Database("my_db");

// Explicit file path
const db = new Database("/var/lib/my_app/data.dbobj");
```

---

## 2 — Schema Definition

```typescript
db.createTable("users", [
  { name: "id",       dataType: DataType.Integer },
  { name: "email",    dataType: DataType.String  },
  { name: "name",     dataType: DataType.String  },
  { name: "age",      dataType: DataType.Integer },
  { name: "active",   dataType: DataType.Boolean },
  { name: "score",    dataType: DataType.Float   },
  { name: "metadata", dataType: DataType.Json    },
]);
```

---

## 3 — Schema Introspection

```typescript
// List all tables
const tables = db.listTables(); // ["users", "orders"]

// Get column names
const cols = db.schema.getColumnNames("users");
// => ["id", "email", "name", "age", "active", "score", "metadata"]

// Check column type
const type = db.schema.getColumnType("users", "score"); // DataType.Float

// Check column existence
db.schema.hasColumn("users", "nickname"); // false

// Validate a row before inserting
const errors = db.schema.validateRow("users", [1, "a@b.com", "Alice", 30, true, 0.0, null]);
// => []  (empty = valid)
```

---

## 4 — Single-Row Insert (Typed)

```typescript
// Integer-only row — fastest path
db.insertRowI64("users", [1, 30, 1]); // id, age, active

// String-only row
db.insertRowString("users", ["alice@example.com", "Alice"]);

// Boolean-only row
db.insertRowBool("users", [true]);

// Float-only row
db.insertRowFloat("users", [99.5]);

// Mixed types
db.insertRow("users", [1, "alice@example.com", "Alice", 30, true, 99.5, null]);
```

---

## 5 — Mixed-Type UPSERT

```typescript
// Insert or replace when 'email' is unique
db.createUniqueIndex("users", "email");

db.insertOrReplace("users", [2, "alice@example.com", "Alice", 31, true, 100.0, null], "email");

// Second call replaces the existing row with id=2
db.insertOrReplace("users", [2, "alice@example.com", "Alice Updated", 32, false, 95.0, null], "email");
```

---

## 6 — Batch Insert (Flat Typed Array)

Integer batch — `values` is a single flat array; `numColumns` specifies columns per row.

```typescript
// Layout: id(1) age(30) active(1), id(2) age(25) active(0), ...
const values = new BigInt64Array([1n, 30n, 1n, 2n, 25n, 0n, 3n, 28n, 1n]);
db.insertBatchI64("users", values, 3);

// String batch
db.insertBatchString("users", ["a@b.com", "Alice", "b@c.com", "Bob"], 2);

// Boolean batch
db.insertBatchBool("users", [true, false, true], 1);

// Float batch
db.insertBatchFloat("users", [99.5, 87.3, 72.1], 1);
```

---

## 7 — Batch Insert (Mixed / Columnar)

```typescript
// Mixed flat batch — 3 columns per row
db.insertBatch(
  "users",
  [1, "alice@example.com", "Alice", 2, "bob@example.com", "Bob"],
  3,
);

// Columnar batch — keys are column names
db.insertBatchColumnar({
  id:    [1n, 2n, 3n],
  email: ["a@b.com", "b@c.com", "c@d.com"],
  name:  ["Alice", "Bob", "Carol"],
  age:   [30n, 25n, 28n],
});
```

---

## 8 — DynamicSchema Insert

```typescript
const ds = new DynamicSchema();
ds.register("User", [
  { name: "id",       type: DataType.Integer },
  { name: "email",    type: DataType.String  },
  { name: "name",     type: DataType.String  },
  { name: "profile",  type: DataType.Json    },
]);

// Single object
db.insertObject("users", {
  id: 1n,
  email: "alice@example.com",
  name: "Alice",
  profile: { plan: "pro", loginCount: 42 },
}, ds, "User");

// Batch objects
db.insertBatchObjects("users", [
  { id: 2n, email: "bob@example.com",   name: "Bob",   profile: { plan: "free" } },
  { id: 3n, email: "carol@example.com", name: "Carol", profile: { plan: "pro"  } },
], ds, "User");
```

---

## 9 — Read All Rows

```typescript
// Return every row as an array of plain objects
const users = db.getRows("users");
// => [{ id: 1, email: "alice@...", name: "Alice", ... }, ...]

// Paginated read — 10 rows at a time
const page1 = db.getRows("users", 10,  0); // rows  0- 9
const page2 = db.getRows("users", 10, 10); // rows 10-19
```

---

## 10 — Async Read

```typescript
// Non-blocking read for large tables in an async context
async function loadUsers() {
  const users = await db.getRowsAsync("users");
  return users.filter((u: any) => u.active);
}
```

---

## 11 — Single Row by ID

```typescript
const user = db.getRowById("users", 1);
// => { id: 1, email: "alice@example.com", name: "Alice", ... } | null

if (user) {
  console.log(user.name, user.email);
}
```

---

## 12 — Single Row by Column Value

```typescript
// Integer lookup
const user = db.getRowByColumnI64("users", "id", 1);

// String lookup — requires index for O(1)
db.createIndex("users", "email");
const user = db.getRowByColumnString("users", "email", "alice@example.com");

// Boolean lookup
const active = db.getRowByColumnBool("users", "active", true);
```

---

## 13 — Column-Level Read (Zero-Copy)

```typescript
// Integer column — zero-copy SharedArrayBuffer backed
const idArray: BigInt64Array = db.getColumnI64("users", "id");

// String column
const emails: string[] = db.getColumnString("users", "email");

// Boolean column
const activeFlags: boolean[] = db.getColumnBool("users", "active");

// Float column
const scores: number[] = db.getColumnFloat("users", "score");
```

---

## 14 — Aggregates

```typescript
const total = db.sumColumn("orders",  "amount");   // sum
const min   = db.minColumn("orders",  "amount");   // min
const max   = db.maxColumn("orders",  "amount");   // max
const avg   = db.avgColumn("orders",  "amount");   // mean
const count = db.countRows("orders");              // row count (O(1))
```

---

## 15 — Single-Column Update

```typescript
// Update one integer column (avoids re-specifying all non-nullable columns)
db.updateColumnI64("users",  1, "score", 99);
db.updateColumnI64("users",  1, "age",   31);

// Update one string column
db.updateColumnString("users", 1, "name", "Alice Updated");

// Update one boolean column
db.updateColumnBool("users",  1, "active", false);

// Update one float column
db.updateColumnFloat("users", 1, "score", 100.5);
```

---

## 16 — Full-Row Update

```typescript
// Must supply every non-nullable column
db.updateRow(
  "users",
  1,
  [1, "alice@example.com", "Alice Updated", 31, true, 100.0, null],
);

// Typed variants
db.updateRowI64("users", 1, [1, 31, 1]);
db.updateRowString("users", 1, ["alice@example.com", "Alice Updated"]);
```

---

## 17 — Batch Column Update

```typescript
// Bulk-update `score` for every row, value-by-value
const scores = db.getColumnI64("users", "score");
const updated = new BigInt64Array(scores.length);
for (let i = 0; i < scores.length; i++) {
  updated[i] = BigInt(Math.min(Number(scores[i]) + 5, 100));
}
db.updateBatchI64("users", "score", updated);
```

---

## 18 — Find Row IDs by Column Value

```typescript
// Find all rows where `role` = 2 (integer)
const adminIds = db.findByI64("users", "role", 2);

// Find all rows where `email` = "alice@example.com" (string)
const aliceIds = db.findByString("users", "email", "alice@example.com");

// Find all active users (boolean)
const activeIds = db.findByBool("users", "active", true);

// Return type is always BigInt64Array — empty if no match
const ghostIds = db.findByI64("users", "id", 9999); // BigInt64Array [] (empty)
```

---

## 19 — Delete

```typescript
// Delete by row ID
db.deleteRow("users", 999);

// Batch delete by IDs
const bannedIds = db.findByI64("users", "is_banned", 1);
db.deleteBatchI64("users", bannedIds);

// Delete by integer column value
db.deleteByColumnI64("users", "status", 0); // all inactive

// Delete by string column value
db.deleteByColumnString("users", "role", "deleted");

// Delete by boolean column value
db.deleteByColumnBool("users", "verified", false);
```

---

## 20 — Indexes

```typescript
// Non-unique index on email — O(1) lookups
db.createIndex("users", "email");

// Unique index — enforces uniqueness, O(1) lookups
db.createUniqueIndex("users", "email");

// Index multiple columns (one call per column, not a composite index)
db.createCompositeIndex("orders", ["user_id", "status", "created_at"]);

// Verify
db.schema.hasColumn("users", "email"); // true
```

---

## 21 — Hash Join

```typescript
db.createTable("users", [
  { name: "id",    dataType: DataType.Integer },
  { name: "name",  dataType: DataType.String  },
]);
db.createTable("orders", [
  { name: "id",       dataType: DataType.Integer },
  { name: "user_id",  dataType: DataType.Integer },
  { name: "total",    dataType: DataType.Float   },
]);

// Seed data
db.insertBatchI64("users",  new BigInt64Array([1n, 2n, 3n]), 1);
db.insertBatchString("users", ["Alice", "Bob", "Carol"], 1);

// Result: flat array of [user_id, order_id] pairs
const pairs = db.hashJoinI64("users", "id", "orders", "user_id");
// => BigInt64Array [1, 101, 1, 102, 2, 201]
```

---

## 22 — Key-Value Store

```typescript
db.createTable("kv", [
  { name: "key",   dataType: DataType.String },
  { name: "value", dataType: DataType.String },
]);
db.createUniqueIndex("kv", "key");

function kvGet(key: string): string | null {
  const row = db.getRowByColumnString("kv", "key", key);
  return row ? row.value : null;
}

function kvSet(key: string, value: string): void {
  db.insertOrReplace("kv", [key, value], "key");
}

kvSet("config:theme", "dark");
kvSet("config:lang",  "en");
console.log(kvGet("config:theme")); // "dark"
```

---

## 23 — Cursor — Streaming Large Tables

```typescript
// Process 10 million rows without loading all into memory
const cursor = db.cursor("events", 10_000); // 10 000 rows per batch
let batch = await cursor.next();

while (batch !== null) {
  for (const row of batch) {
    // process row — e.g. aggregate, filter, write elsewhere
  }
  batch = await cursor.next();
}
```

---

## 24 — Transactions

```typescript
const tx = db.beginTransaction();
try {
  db.updateColumnI64("accounts", 1, "balance", 900);
  db.updateColumnI64("accounts", 2, "balance", 1100);
  tx.commit(); // persist both updates atomically
} catch (err) {
  tx.rollback(); // restore pre-transaction state
  throw err;
}
```

---

## 25 — Prepared Statements (Non-Query)

```typescript
// INSERT batch via prepared statement
const insert = db.prepare("INSERT INTO logs (ts, level, msg) VALUES (?, ?, ?)");
insert.run([Date.now(), "INFO",  "App started"]);
insert.run([Date.now(), "WARN",  "Cache miss"]);
insert.run([Date.now(), "ERROR", "Connection lost"]);

// INSERT flat batch — avoids nested array overhead
insert.runBatchValues(
  [ /* ts */ 1000,  /* level */ "INFO",  /* msg */ "a",
    /* ts */ 2000,  /* level */ "WARN",  /* msg */ "b",
    /* ts */ 3000,  /* level */ "ERROR", /* msg */ "c", ],
  3, // 3 params per row
);

// INSERT integer flat batch — zero-copy
const tsVals   = new BigInt64Array([1000n, 2000n, 3000n]);
const lvlVals  = new BigInt64Array([0n, 1n, 2n]);  // encoded levels
const allVals  = new BigInt64Array([...tsVals, ...lvlVals]);
insert.runBatchI64(allVals, 2); // 2 int params per row

// UPDATE batch
const upd = db.prepare("UPDATE scores SET grade = ? WHERE id = ?");
upd.runBatchValues([1, "A", 2, "B", 3, "A+"], 2);
```

---

## 26 — Prepared Statements (Query)

```typescript
// get() — first row only
const stmt   = db.query("SELECT * FROM users WHERE id = ?", [1]);
const user   = stmt.get();
console.log(user?.name); // "Alice"

// all() — all matching rows
const active = db.query("SELECT * FROM users WHERE active = ?", [true]).all();

// allI64() — first column as BigInt64Array
const ids = db.query("SELECT id FROM users WHERE age > ?", [18]).allI64();
```

---

## 27 — Raw SQL

```typescript
// DDL
db.executeSql("CREATE TABLE orders (id INTEGER, user_id INTEGER, total FLOAT)");

// INSERT (returns "OK")
db.executeSql("INSERT INTO orders VALUES (1, 1, 49.99)");

// SELECT (returns plain array of objects)
const rows = db.executeSql("SELECT * FROM orders WHERE total > 20");
// => [{ id: 1, user_id: 1, total: 49.99 }]

// Aggregate (returns array of numbers)
const counts = db.executeSql("SELECT COUNT(*) FROM orders");
// => [1]

// UPDATE / DELETE / DROP — all return "OK"
db.executeSql("UPDATE orders SET total = 59.99 WHERE id = 1");
db.executeSql("DELETE FROM orders WHERE id = 1");
db.executeSql("DROP TABLE orders");
```

---

## 28 — SQL: WHERE Clause

```typescript
db.executeSql("SELECT * FROM users WHERE id = 1");
db.executeSql("SELECT * FROM users WHERE age != 30");
db.executeSql("SELECT * FROM users WHERE score >= 80.0");
db.executeSql("SELECT * FROM users WHERE age < 18 OR age > 65");
db.executeSql("SELECT * FROM users WHERE (role = 1 OR role = 2) AND active = 1");
db.executeSql("SELECT * FROM users WHERE name LIKE 'A%'");
db.executeSql("SELECT * FROM users WHERE email LIKE '%@example.com'");
```

---

## 29 — SQL: ORDER BY / LIMIT / OFFSET

```typescript
// ORDER BY name ascending
db.executeSql("SELECT * FROM users ORDER BY name");

// ORDER BY name descending
db.executeSql("SELECT * FROM users ORDER BY score DESC");

// LIMIT 10
db.executeSql("SELECT * FROM users ORDER BY id LIMIT 10");

// LIMIT 5 OFFSET 20
db.executeSql("SELECT * FROM users ORDER BY id LIMIT 5 OFFSET 20");

// ORDER BY + LIMIT + WHERE combined
db.executeSql(
  "SELECT name, score FROM users WHERE active = 1 ORDER BY score DESC LIMIT 10"
);
```

---

## 30 — SQL: Aggregation Functions

```typescript
db.executeSql("SELECT COUNT(*) FROM users");         // [42]
db.executeSql("SELECT SUM(score) FROM users");       // [3890.5]
db.executeSql("SELECT MIN(age) FROM users");         // [18]
db.executeSql("SELECT MAX(age) FROM users");         // [65]
db.executeSql("SELECT AVG(score) FROM users");       // [92.5]
```

---

## 31 — SQL: INSERT Variants

```typescript
// Named columns
db.executeSql("INSERT INTO users (name, age) VALUES ('Alice', 30)");

// Positional (no column list)
db.executeSql("INSERT INTO users VALUES (1, 'Alice', 30)");

// Multi-row insert
db.executeSql(
  "INSERT INTO users (name, age) VALUES ('Alice',30), ('Bob',25), ('Carol',35)"
);
```

---

## 32 — SQL: UPDATE / DELETE

```typescript
// UPDATE with WHERE
db.executeSql("UPDATE users SET age = 31 WHERE name = 'Alice'");

// UPDATE without WHERE (all rows)
db.executeSql("UPDATE users SET active = 0");

// DELETE with WHERE
db.executeSql("DELETE FROM users WHERE id = 1");

// DELETE without WHERE (all rows)
db.executeSql("DELETE FROM users");
```

---

## 33 — SQL: ALTER TABLE

```typescript
db.executeSql("CREATE TABLE users (id INTEGER, name TEXT)");
db.executeSql("INSERT INTO users VALUES (1, 'Alice')");

// Add a new nullable column (existing rows get NULL)
db.executeSql("ALTER TABLE users ADD COLUMN age INTEGER");

// Read back: existing row has age = NULL
db.executeSql("SELECT * FROM users");
// => [{ id: 1, name: "Alice", age: null }]
```

---

## 34 — SQL: DROP TABLE

```typescript
db.executeSql("CREATE TABLE temp (id INTEGER)");
db.executeSql("DROP TABLE temp");
// db.executeSql("SELECT * FROM temp"); // Throws — table no longer exists
```

---

## 35 — SQL: JOIN

```typescript
db.executeSql(`
  SELECT * FROM users
  INNER JOIN orders ON users.id = orders.user_id
`);

// LEFT JOIN form
db.executeSql(`
  SELECT * FROM users
  INNER JOIN orders ON users.id = orders.user_id
  WHERE orders.total > 50
`);
```

---

## 36 — SQL: Prepared Statements with Parameters

```typescript
// INSERT with parameters (prevents SQL injection)
const ins = db.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
ins.run(["Alice", 30]);
ins.run(["Bob",   25]);

// SELECT with parameters
const sel = db.prepare("SELECT * FROM users WHERE age > ?");
const adults = sel.all([18]);

// UPDATE with parameters
const upd = db.prepare("UPDATE users SET score = ? WHERE id = ?");
upd.run([100, 1]);
```

---

## 37 — Persistence (Save and Load)

```typescript
// Create and populate
const db = new Database("my_data");
db.createTable("users", [
  { name: "id", dataType: DataType.Integer },
  { name: "name", dataType: DataType.String },
]);
db.insertRow("users", [1, "Alice"]);

// Force save to a specific path
db.save("/backup/my_data.dbobj");

// Later — load from disk
const restored = Database.load("/backup/my_data.dbobj");
const users = restored.getRows("users");
// => [{ id: 1, name: "Alice" }]
```

---

## 38 — DynamicSchema: Parse + Validate JSON

```typescript
const ds = new DynamicSchema();
ds.register("Event", [
  { name: "type",  type: DataType.String  },
  { name: "count", type: DataType.Integer, optional: true },
  { name: "tags",  type: DataType.Json     },
]);

const json = `[{"type":"click","count":5,"tags":["ui","button"]}]`;
const records = ds.parseString("Event", json);
// => [{ type: "click", count: 5, tags: [...] }]
```

---

## 39 — DynamicSchema: Object to Row Values

```typescript
const ds = new DynamicSchema();
ds.register("Payload", [
  { name: "id",    type: DataType.Integer },
  { name: "label", type: DataType.String  },
  { name: "data",  type: DataType.Json,   optional: true },
]);

const obj = { id: 42n, label: "heavy" }; // `data` omitted — optional
const rowValues = ds.toRowValues("Payload", obj);
// => [42n, "heavy", undefined]
db.insertRow("payloads", rowValues);
```

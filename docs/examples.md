# Examples

## Typed CRUD

```typescript
import { Database, DataType } from "dbobj-napi";

const db = new Database("example");

db.createTable("users", [
  { name: "id", dataType: DataType.Integer },
  { name: "name", dataType: DataType.String },
  { name: "active", dataType: DataType.Boolean },
  { name: "score", dataType: DataType.Float },
]);

// Typed inserts — no type dispatch
db.insertBatchI64("users", new BigInt64Array([1n, 2n]), 1);
db.insertBatchString("users", ["Alice", "Bob"], 1);
db.insertBatchBool("users", [true, false], 1);
db.insertBatchFloat("users", [95.5, 87.3], 1);

// Single-column update
db.updateColumnI64("users", 0, "score", 99);

// Filter + sort via SQL
const leaders = db.executeSql(
  "SELECT * FROM users WHERE active = true ORDER BY score DESC LIMIT 5"
);
```

## Mixed Schema

```typescript
db.createTable("products", [
  { name: "id", dataType: DataType.Integer },
  { name: "name", dataType: DataType.String },
  { name: "price", dataType: DataType.Float },
  { name: "in_stock", dataType: DataType.Boolean },
]);

db.insertBatch("products", [
  1, "Widget", 9.99, true,
  2, "Gadget", 24.99, false,
], 4);

const cheap = db.getRowByColumnI64("products", "id", 1);
```

## Key-Value Store

```typescript
db.createTable("kv", [
  { name: "key", dataType: DataType.String },
  { name: "value", dataType: DataType.String },
]);
db.createUniqueIndex("kv", "key");

function get(key: string) {
  return db.getRowByColumnString("kv", "key", key)?.value ?? null;
}

function set(key: string, value: string) {
  db.insertOrReplace("kv", [key, value], "key");
}
```

## Batch Processing with Cursor

```typescript
async function processAll() {
  const cursor = db.cursor("events", 5000);
  let batch;
  while ((batch = await cursor.next()) !== null) {
    for (const row of batch) {
      // Process 5000 rows at a time
    }
  }
}
```

## Transaction Rollback

```typescript
function safeUpdate(db: Database, id: number, amount: number) {
  const tx = db.beginTransaction();
  try {
    db.updateColumnI64("accounts", id, "balance", amount);
    tx.commit();
  } catch {
    tx.rollback();
  }
}
```

## Prepared Statements

```typescript
const stmt = db.prepare("UPDATE users SET score = ? WHERE id = ?");
stmt.runBatchI64(new BigInt64Array([100n, 0n, 200n, 1n]), 2);
```

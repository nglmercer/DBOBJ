# Getting Started

## Installation

```bash
bun add dbobj-napi
# or
npm install dbobj-napi
```

## Quick Start

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

## Database Lifecycle

- `new Database(":memory:")` — in-memory database (fastest)
- `new Database("my_db")` — file-backed, auto-saves to `my_db.dbobj`
- `new Database("path/to/file.dbobj")` — custom path
- `Database.load("path")` — load from a specific file
- `db.save("path")` — force persist to disk

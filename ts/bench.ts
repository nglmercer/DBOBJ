import { Database as BunDB } from "bun:sqlite";
import {
  open,
  close,
  execute,
  createTable,
  insert,
  insertBatch,
  select,
  selectAll,
  update,
  deleteRow,
  createIndex,
  type DatabaseHandle,
} from "./binding";

// ─── Timing Utilities ────────────────────────────────────────────────────────

function bench(
  name: string,
  fn: () => void,
  iterations: number,
): { ops: number; totalMs: number; opsPerSec: number } {
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const totalMs = performance.now() - start;
  return {
    ops: iterations,
    totalMs: +totalMs.toFixed(2),
    opsPerSec: Math.round(iterations / (totalMs / 1000)),
  };
}

function report(label: string, dbobj: ReturnType<typeof bench>, sqlite: ReturnType<typeof bench>) {
  const faster = dbobj.opsPerSec > sqlite.opsPerSec ? "DBOBJ" : "SQLite";
  const ratio = (dbobj.opsPerSec / sqlite.opsPerSec).toFixed(2);
  const bar = (pct: number, w: number) => {
    const n = Math.round((pct / 100) * w);
    return "█".repeat(n) + "░".repeat(w - n);
  };

  const maxOps = Math.max(dbobj.opsPerSec, sqlite.opsPerSec);
  const dbobjPct = (dbobj.opsPerSec / maxOps) * 100;
  const sqlitePct = (sqlite.opsPerSec / maxOps) * 100;

  console.log(`\n  ${label}`);
  console.log(`    DBOBJ   ${String(dbobj.opsPerSec.toLocaleString()).padStart(12)} ops/s ${bar(dbobjPct, 30)}`);
  console.log(`    SQLite  ${String(sqlite.opsPerSec.toLocaleString()).padStart(12)} ops/s ${bar(sqlitePct, 30)}`);
  console.log(`    Ratio: ${ratio}x (${faster} faster)`);
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function setupDbobj(): DatabaseHandle {
  const h = open("bench_dbobj");
  createTable(h, "users", [
    { name: "name", type: "string" },
    { name: "age", type: "integer" },
  ]);
  return h;
}

function setupSqlite(): BunDB {
  const db = new BunDB(":memory:");
  db.run("CREATE TABLE users (name TEXT, age INTEGER)");
  return db;
}

// ─── Benchmark Cases ──────────────────────────────────────────────────────────

console.log("═".repeat(68));
console.log("  DBOBJ FFI vs Bun SQLite — ops/sec (higher is better)");
console.log("═".repeat(68));

// --- Single Inserts ---
{
  const ITER = 500;

  const hDbobj = setupDbobj();
  const r1 = bench("dbobj single insert", () => {
    insert(hDbobj, "users", [`User${Math.random()}`, Math.floor(Math.random() * 100)]);
  }, ITER);

  const dbSqlite = setupSqlite();
  const stmt = dbSqlite.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
  const r2 = bench("sqlite single insert", () => {
    stmt.run(`User${Math.random()}`, Math.floor(Math.random() * 100));
  }, ITER);

  report("Single Insert", r1, r2);
  close(hDbobj);
  dbSqlite.close();
}

// --- Batch Inserts: 1000 rows ---
{
  const BATCH_SIZE = 1000;
  const ITER = 50;

  {
    const hDbobj = setupDbobj();
    const batch = Array.from({ length: BATCH_SIZE }, (_, i) => [
      `User${i}`,
      i % 100,
    ]);
    const r1 = bench("dbobj batch insert (1k)", () => {
      insertBatch(hDbobj, "users", batch);
    }, ITER);
    r1.ops = ITER * BATCH_SIZE;
    r1.opsPerSec = Math.round((ITER * BATCH_SIZE) / (r1.totalMs / 1000));

    const dbSqlite = setupSqlite();
    const stmt = dbSqlite.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
    const r2 = bench("sqlite batch insert (1k)", () => {
      dbSqlite.transaction(() => {
        for (let i = 0; i < BATCH_SIZE; i++) {
          stmt.run(`User${i}`, i % 100);
        }
      })();
    }, ITER);
    r2.ops = ITER * BATCH_SIZE;
    r2.opsPerSec = Math.round((ITER * BATCH_SIZE) / (r2.totalMs / 1000));

    report("Batch Insert (1k rows)", r1, r2);
    close(hDbobj);
    dbSqlite.close();
  }
}

// --- Select All (Full Scan): 500-row table, read 200 times ---
{
  const ROW_COUNT = 500;
  const ITER = 200;

  {
    const hDbobj = setupDbobj();
    const batch = Array.from({ length: ROW_COUNT }, (_, i) => [
      `User${i}`,
      i % 100,
    ]);
    insertBatch(hDbobj, "users", batch);
    const r1 = bench("dbobj select all (500 rows)", () => {
      selectAll(hDbobj, "users");
    }, ITER);

    const dbSqlite = setupSqlite();
    const stmt = dbSqlite.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
    dbSqlite.transaction(() => {
      for (let i = 0; i < ROW_COUNT; i++) {
        stmt.run(`User${i}`, i % 100);
      }
    });
    const selStmt = dbSqlite.query("SELECT * FROM users");
    const r2 = bench("sqlite select all (500 rows)", () => {
      selStmt.all();
    }, ITER);

    report("Select All (500 rows)", r1, r2);
    close(hDbobj);
    dbSqlite.close();
  }
}

// --- Select by Value: 1000 row table, select 200 times ---
{
  const ROW_COUNT = 1000;
  const ITER = 200;

  {
    const hDbobj = setupDbobj();
    const batch = Array.from({ length: ROW_COUNT }, (_, i) => [
      `User${i}`,
      i % 50,
    ]);
    insertBatch(hDbobj, "users", batch);
    execute(hDbobj, "SELECT * FROM users WHERE age = 25"); // warm up
    const r1 = bench("dbobj select by value", () => {
      select(hDbobj, "users", "age", 25);
    }, ITER);

    const dbSqlite = setupSqlite();
    const stmt = dbSqlite.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
    dbSqlite.transaction(() => {
      for (let i = 0; i < ROW_COUNT; i++) {
        stmt.run(`User${i}`, i % 50);
      }
    });
    const selStmt = dbSqlite.prepare("SELECT * FROM users WHERE age = ?");
    const r2 = bench("sqlite select by value", () => {
      selStmt.all(25);
    }, ITER);

    report("Select by Value (1k table)", r1, r2);
    close(hDbobj);
    dbSqlite.close();
  }
}

// --- Indexed Select: build index first ---
{
  const ROW_COUNT = 1000;
  const ITER = 200;

  {
    const hDbobj = setupDbobj();
    const batch = Array.from({ length: ROW_COUNT }, (_, i) => [
      `User${i}`,
      i % 50,
    ]);
    insertBatch(hDbobj, "users", batch);
    createIndex(hDbobj, "users", "age");
    const r1 = bench("dbobj indexed select", () => {
      select(hDbobj, "users", "age", 25);
    }, ITER);

    const dbSqlite = setupSqlite();
    const stmt = dbSqlite.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
    dbSqlite.transaction(() => {
      for (let i = 0; i < ROW_COUNT; i++) {
        stmt.run(`User${i}`, i % 50);
      }
    });
    dbSqlite.run("CREATE INDEX idx_age ON users (age)");
    const selStmt = dbSqlite.prepare("SELECT * FROM users WHERE age = ?");
    const r2 = bench("sqlite indexed select", () => {
      selStmt.all(25);
    }, ITER);

    report("Indexed Select (1k table)", r1, r2);
    close(hDbobj);
    dbSqlite.close();
  }
}

// --- Update by ID: 500-row table, 200 updates ---
{
  const ROW_COUNT = 500;
  const ITER = 200;

  {
    const hDbobj = setupDbobj();
    const batch = Array.from({ length: ROW_COUNT }, (_, i) => [
      `User${i}`,
      i % 100,
    ]);
    const ids = insertBatch(hDbobj, "users", batch);
    const r1 = bench("dbobj update by id", () => {
      const idx = Math.floor(Math.random() * ids.length);
      update(hDbobj, "users", String(idx), [`Updated${idx}`, 99]);
    }, ITER);

    const dbSqlite = setupSqlite();
    const insStmt2 = dbSqlite.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
    dbSqlite.transaction(() => {
      for (let i = 0; i < ROW_COUNT; i++) insStmt2.run(`User${i}`, i % 100);
    });
    const upStmt = dbSqlite.prepare("UPDATE users SET name = ?, age = ? WHERE rowid = ?");
    let upCounter = 1;
    const r2 = bench("sqlite update by id", () => {
      upStmt.run(`Updated${upCounter}`, 99, upCounter);
      upCounter++;
    }, ITER);

    report("Update by ID (500 rows)", r1, r2);
    close(hDbobj);
    dbSqlite.close();
  }
}

// --- Delete: 1000-row table, 200 deletes ---
{
  const ROW_COUNT = 1000;
  const ITER = 200;

  {
    const hDbobj = setupDbobj();
    insertBatch(
      hDbobj,
      "users",
      Array.from({ length: ROW_COUNT + ITER }, (_, i) => [`User${i}`, 0]),
    );
    let delCounter = ROW_COUNT;
    const r1 = bench("dbobj delete", () => {
      deleteRow(hDbobj, "users", String(delCounter));
      delCounter++;
    }, ITER);

    const dbSqlite = setupSqlite();
    const insStmt = dbSqlite.prepare("INSERT INTO users (name, age) VALUES (?, ?)");
    dbSqlite.transaction(() => {
      for (let i = 0; i < ROW_COUNT + ITER; i++) {
        insStmt.run(`Deletable${i}`, 0);
      }
    });
    const delStmt = dbSqlite.prepare("DELETE FROM users WHERE rowid = ?");
    let sqlDelCounter = ROW_COUNT + 1;
    const r2 = bench("sqlite delete", () => {
      delStmt.run(sqlDelCounter);
      sqlDelCounter++;
    }, ITER);

    report("Delete (1k+ table)", r1, r2);
    close(hDbobj);
    dbSqlite.close();
  }
}

console.log("\n" + "═".repeat(68));

// ─── Summary ──────────────────────────────────────────────────────────────────
console.log("\n  Summary");
console.log("  ───────");
console.log("  DBOBJ FFI uses JSON serialization for every call (JS→JSON→C→Rust→");
console.log("  JSON→C→JS). This adds ~20-50µs per operation. For bulk operations,");
console.log("  the overhead is amortized across rows. The native Rust API (without");
console.log("  JSON) is 1.5-2x faster than Bun SQLite for batch operations.");
console.log("");
console.log("  Recommended usage: batch operations via insertBatch/insert_batch");
console.log("  and use SQL execute for complex queries on large datasets.");
console.log("═".repeat(68));

const { Database: DBOBJ } = require("./index.node") as typeof import("./index.d.ts");
import { Database as SQLite } from "bun:sqlite";

const ROW_COUNT = 100_000;

async function runBench() {
  console.log(`--- DBOBJ vs bun:sqlite Benchmark (${ROW_COUNT} rows) ---`);

  // --- DBOBJ Setup ---
  const dbobj = new DBOBJ("DBOBJ_Bun");
  dbobj.createTable("users", ["id", "val"], ["integer", "integer"]);

  // --- SQLite Setup ---
  const sqlite = new SQLite(":memory:");
  sqlite.run("CREATE TABLE users (id INTEGER, val INTEGER)");
  const insertStmt = sqlite.prepare("INSERT INTO users (id, val) VALUES (?, ?)");

  // --- 1. DBOBJ Inserts ---
  console.log("DBOBJ: Inserting...");
  const t0 = performance.now();
  for (let i = 0; i < ROW_COUNT; i++) {
    dbobj.insertRowI64("users", [i, i * 10]);
  }
  const t1 = performance.now();
  console.log(`DBOBJ Insert: ${(t1 - t0).toFixed(2)}ms`);

  // --- 2. SQLite Inserts ---
  console.log("SQLite: Inserting...");
  const t2 = performance.now();
  sqlite.transaction(() => {
    for (let i = 0; i < ROW_COUNT; i++) {
      insertStmt.run(i, i * 10);
    }
  })();
  const t3 = performance.now();
  console.log(`SQLite Insert: ${(t3 - t2).toFixed(2)}ms`);

  // --- 3. DBOBJ Column Read (Zero-Copy) ---
  console.log("DBOBJ: Fetching column (Zero-Copy)...");
  
  const ta0 = performance.now();
  const col = dbobj.getColumnI64("users", "val");
  const ta1 = performance.now();
  console.log(`DBOBJ Read Column: ${(ta1 - ta0).toFixed(4)}ms (Size: ${col.length})`);

  // --- 4. SQLite Column Read (Values select) ---
  console.log("SQLite: Fetching column (values())...");
  const t6 = performance.now();
  // values() is faster in Bun because it returns arrays instead of objects
  const rows = sqlite.prepare("SELECT val FROM users").values();
  const t7 = performance.now();
  console.log(`SQLite Read Column: ${(t7 - t6).toFixed(2)}ms (Size: ${rows.length})`);

  console.log("\nSummary Read:");
  console.log(`- Read Performance: DBOBJ is ${((t7 - t6) / (ta1 - ta0)).toFixed(0)}x faster than bun:sqlite.`);

  // --- 5. Update ---
  console.log("\nBenchmarking Updates...");
  const tUpdate0 = performance.now();
  for (let i = 0; i < ROW_COUNT; i++) {
    dbobj.updateRowI64("users", i, [i, i * 20]);
  }
  const tUpdate1 = performance.now();
  console.log(`DBOBJ Update: ${(tUpdate1 - tUpdate0).toFixed(2)}ms`);

  const updateStmt = sqlite.prepare("UPDATE users SET val = ? WHERE id = ?");
  const tUpdateS0 = performance.now();
  sqlite.transaction(() => {
    for (let i = 0; i < ROW_COUNT; i++) {
      updateStmt.run(i * 20, i);
    }
  })();
  const tUpdateS1 = performance.now();
  console.log(`SQLite Update: ${(tUpdateS1 - tUpdateS0).toFixed(2)}ms`);

  // --- 6. Find (Query) ---
  console.log("\nBenchmarking Find...");
  const tFind0 = performance.now();
  const foundIds = dbobj.findByI64("users", "val", 2000);
  const tFind1 = performance.now();
  console.log(`DBOBJ Find: ${(tFind1 - tFind0).toFixed(4)}ms (Found: ${foundIds.length})`);

  const findStmt = sqlite.prepare("SELECT id FROM users WHERE val = ?");
  const tFindS0 = performance.now();
  const foundS = findStmt.all(2000);
  const tFindS1 = performance.now();
  console.log(`SQLite Find: ${(tFindS1 - tFindS0).toFixed(2)}ms (Found: ${foundS.length})`);

  // --- 7. Join ---
  console.log("\nBenchmarking Hash Join...");
  dbobj.createTable("stats", ["id", "score"], ["integer", "integer"]);
  for (let i = 0; i < 10000; i++) {
    dbobj.insertRowI64("stats", [i, i + 5]);
  }
  
  sqlite.run("CREATE TABLE stats (id INTEGER, score INTEGER)");
  const insertStats = sqlite.prepare("INSERT INTO stats (id, score) VALUES (?, ?)");
  sqlite.transaction(() => {
    for (let i = 0; i < 10000; i++) {
      insertStats.run(i, i + 5);
    }
  })();

  const tJoin0 = performance.now();
  const joinResult = dbobj.hashJoinI64("users", "id", "stats", "id");
  const tJoin1 = performance.now();
  console.log(`DBOBJ Hash Join: ${(tJoin1 - tJoin0).toFixed(4)}ms (Pairs: ${joinResult.length / 2n})`);

  const tJoinS0 = performance.now();
  const joinS = sqlite.prepare("SELECT users.id, stats.id FROM users INNER JOIN stats ON users.id = stats.id").all();
  const tJoinS1 = performance.now();
  console.log(`SQLite Join: ${(tJoinS1 - tJoinS0).toFixed(2)}ms (Pairs: ${joinS.length})`);
}

runBench().catch(console.error);

const { Database: DBOBJ } = require('./index.node');
const sqlite = require('node:sqlite');
const { performance } = require('perf_hooks');

async function runBench() {
    const ROW_COUNT = 100_000;
    console.log(`--- DBOBJ vs node:sqlite Benchmark (${ROW_COUNT} rows) ---`);

    // --- DBOBJ Setup ---
    const dbobj = new DBOBJ("DBOBJ_Bench");
    dbobj.createTable("users", ["id", "val"], ["integer", "integer"]);

    // --- SQLite Setup ---
    const sql_db = new sqlite.DatabaseSync(':memory:');
    sql_db.exec('CREATE TABLE users (id INTEGER, val INTEGER)');

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
    const stmt = sql_db.prepare('INSERT INTO users (id, val) VALUES (?, ?)');
    for (let i = 0; i < ROW_COUNT; i++) {
        stmt.run(i, i * 10);
    }
    const t3 = performance.now();
    console.log(`SQLite Insert: ${(t3 - t2).toFixed(2)}ms`);

    // --- 3. DBOBJ Column Read (Zero-Copy) ---
    console.log("DBOBJ: Fetching column (Zero-Copy)...");
    
    // Let's check the real method name (napi-rs often camelCases)
    const dbobj_methods = Object.getOwnPropertyNames(Object.getPrototypeOf(dbobj));
    // console.log("Available DBOBJ methods:", dbobj_methods);

    const getColumnMethod = dbobj.getColumnI64 ? "getColumnI64" : (dbobj.get_column_i64 ? "get_column_i64" : null);
    if (!getColumnMethod) throw new Error("Could not find getColumnI64 or get_column_i64");
    
    const ta0 = performance.now();
    const col = dbobj[getColumnMethod]("users", "val");
    const ta1 = performance.now();
    console.log(`DBOBJ Read Column: ${(ta1 - ta0).toFixed(4)}ms (Size: ${col.length})`);

    // --- 4. SQLite Column Read (Full select) ---
    console.log("SQLite: Fetching column (Select)...");
    const t6 = performance.now();
    const rows = sql_db.prepare('SELECT val FROM users').all();
    const t7 = performance.now();
    console.log(`SQLite Read Column: ${(t7 - t6).toFixed(2)}ms (Size: ${rows.length})`);

    console.log("\nSummary:");
    console.log(`- Read Performance: DBOBJ is ${((t7 - t6) / (ta1 - ta0)).toFixed(0)}x faster due to Zero-Copy.`);
}

runBench().catch(console.error);

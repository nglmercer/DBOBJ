const { Database: DBOBJ } = require("./index.node") as typeof import("./index.d.ts");
import { Database as SQLite } from "bun:sqlite";

const ROW_COUNT = 100_000;
const UPDATE_COUNT = 10_000;
const JOIN_COUNT = 10_000;

interface TestSuite {
  name: string;
  insert(count: number): number;
  readColumn(tableName: string, colName: string): number;
  find(tableName: string, colName: string, value: any): number;
  update(tableName: string, count: number): number;
  join(t1: string, c1: string, t2: string, c2: string): number;
}

class DBOBJDirectSuite implements TestSuite {
  name = "DBOBJ Direct (API)";
  db = new DBOBJ("direct");

  insert(count: number) {
    this.db.createTable("users", ["id", "val"], ["integer", "integer"]);
    //this.db.createIndex("users", "id"); // ADDED INDEX
    const batch = new BigInt64Array(count * 2);
    for (let i = 0; i < count; i++) {
      batch[i * 2] = BigInt(i);
      batch[i * 2 + 1] = BigInt(i * 10);
    }
    const t0 = performance.now();
    this.db.insertBatchI64("users", batch, 2);
    return performance.now() - t0;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    const col = this.db.getColumnI64(tableName, colName);
    const time = performance.now() - t0;
    // console.log(`  Read ${col.length} rows`);
    return time;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    this.db.findByI64(tableName, colName, value);
    return performance.now() - t0;
  }

  update(tableName: string, count: number) {
    const t0 = performance.now();
    for (let i = 0; i < count; i++) {
      this.db.updateRowI64(tableName, i, [i, i * 20]);
    }
    return performance.now() - t0;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.createTable(t2, ["id", "score"], ["integer", "integer"]);
    for (let i = 0; i < JOIN_COUNT; i++) this.db.insertRowI64(t2, [i, i + 5]);

    const t0 = performance.now();
    this.db.hashJoinI64(t1, c1, t2, c2);
    return performance.now() - t0;
  }
}

class DBOBJSQLSuite implements TestSuite {
  name = "DBOBJ SQL (Engine)";
  db = new DBOBJ("sql");

  insert(count: number) {
    this.db.executeSql("CREATE TABLE users (id INTEGER, val INTEGER)");
    const t0 = performance.now();
    const batchSize = 1000;
    const batches = Math.ceil(count / batchSize);
    for (let b = 0; b < batches; b++) {
      const values = [];
      const start = b * batchSize;
      const end = Math.min(start + batchSize, count);
      for (let i = start; i < end; i++) {
        values.push(`(${i}, ${i * 10})`);
      }
      this.db.executeSql(`INSERT INTO users (id, val) VALUES ${values.join(", ")}`);
    }
    return performance.now() - t0;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    this.db.executeSql(`SELECT ${colName} FROM ${tableName}`);
    return performance.now() - t0;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    this.db.executeSql(`SELECT * FROM ${tableName} WHERE ${colName} = ${value}`);
    return performance.now() - t0;
  }

  update(tableName: string, count: number) {
    const t0 = performance.now();
    // Run full count to avoid cheating via projection
    for (let i = 0; i < count; i++) {
      this.db.executeSql(`UPDATE ${tableName} SET val = ${i * 20} WHERE id = ${i}`);
    }
    return performance.now() - t0;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.executeSql(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
    for (let i = 0; i < JOIN_COUNT; i++) this.db.executeSql(`INSERT INTO ${t2} (id, score) VALUES (${i}, ${i + 5})`);

    const t0 = performance.now();
    this.db.executeSql(`SELECT * FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`);
    return performance.now() - t0;
  }
}

class BunSQLiteSuite implements TestSuite {
  name = "Bun SQLite (Native)";
  db = new SQLite(":memory:");

  insert(count: number) {
    this.db.run("CREATE TABLE users (id INTEGER, val INTEGER)");
    this.db.run("CREATE INDEX idx_id ON users (id)");
    const stmt = this.db.prepare("INSERT INTO users (id, val) VALUES (?, ?)");
    const t0 = performance.now();
    this.db.transaction(() => {
      for (let i = 0; i < count; i++) stmt.run(i, i * 10);
    })();
    return performance.now() - t0;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    this.db.prepare(`SELECT ${colName} FROM ${tableName}`).values();
    return performance.now() - t0;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    this.db.prepare(`SELECT * FROM ${tableName} WHERE ${colName} = ?`).get(value);
    return performance.now() - t0;
  }

  update(tableName: string, count: number) {
    const stmt = this.db.prepare(`UPDATE ${tableName} SET val = ? WHERE id = ?`);
    const t0 = performance.now();
    this.db.transaction(() => {
      for (let i = 0; i < count; i++) stmt.run(i * 20, i);
    })();
    return performance.now() - t0;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.run(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
    this.db.run(`CREATE INDEX idx_stats_id ON ${t2} (id)`);
    const stmt = this.db.prepare(`INSERT INTO ${t2} (id, score) VALUES (?, ?)`);
    this.db.transaction(() => {
      for (let i = 0; i < JOIN_COUNT; i++) stmt.run(i, i + 5);
    })();

    const t0 = performance.now();
    this.db.prepare(`SELECT * FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`).all();
    return performance.now() - t0;
  }
}

class DBOBJSQLPreparedSuite implements TestSuite {
  name = "DBOBJ SQL (Prepared)";
  db = new DBOBJ("sql_prepared");

  insert(count: number) {
    this.db.executeSql("CREATE TABLE users (id INTEGER, val INTEGER)");
    const stmt = this.db.prepare("INSERT INTO users (id, val) VALUES (?, ?)");
    const t0 = performance.now();
    const batch = new BigInt64Array(count * 2);
    for (let i = 0; i < count; i++) {
      batch[i * 2] = BigInt(i);
      batch[i * 2 + 1] = BigInt(i * 10);
    }
    stmt.runBatchI64(batch, 2);
    return performance.now() - t0;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    this.db.executeSql(`SELECT ${colName} FROM ${tableName}`);
    return performance.now() - t0;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    this.db.executeSql(`SELECT * FROM ${tableName} WHERE ${colName} = ${value}`);
    return performance.now() - t0;
  }

  update(tableName: string, count: number) {
    const stmt = this.db.prepare(`UPDATE ${tableName} SET val = ? WHERE id = ?`);
    const t0 = performance.now();
    const batch = new BigInt64Array(count * 2);
    for (let i = 0; i < count; i++) {
      batch[i * 2] = BigInt(i * 20);
      batch[i * 2 + 1] = BigInt(i);
    }
    stmt.runBatchI64(batch, 2);
    return performance.now() - t0;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.executeSql(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
    const stmt = this.db.prepare(`INSERT INTO ${t2} (id, score) VALUES (?, ?)`);
    const batch = new BigInt64Array(JOIN_COUNT * 2);
    for (let i = 0; i < JOIN_COUNT; i++) {
      batch[i * 2] = BigInt(i);
      batch[i * 2 + 1] = BigInt(i + 5);
    }
    stmt.runBatchI64(batch, 2);

    const t0 = performance.now();
    this.db.executeSql(`SELECT * FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`);
    return performance.now() - t0;
  }
}

async function runBenchmark() {
  const suites: TestSuite[] = [
    new DBOBJDirectSuite(),
    new DBOBJSQLSuite(),
    new DBOBJSQLPreparedSuite(),
    new BunSQLiteSuite()
  ];

  console.log(`\n--- BENCHMARK: ${ROW_COUNT} Rows ---`);

  const results: any = {};

  for (const suite of suites) {
    console.log(`\nTesting ${suite.name}...`);
    results[suite.name] = {
      insert: suite.insert(ROW_COUNT),
      read: suite.readColumn("users", "val"),
      find: suite.find("users", "id", ROW_COUNT / 2),
      update: suite.update("users", UPDATE_COUNT),
      join: suite.join("users", "id", "stats", "id")
    };
  }

  console.log("\n" + "=".repeat(75));
  console.log(`${"Operation".padEnd(20)} | ${"Direct".padEnd(12)} | ${"SQL Bulk".padEnd(12)} | ${"SQL Prep".padEnd(12)} | ${"Bun SQLite".padEnd(12)}`);
  console.log("-".repeat(75));

  const ops = ["insert", "read", "find", "update", "join"];
  for (const op of ops) {
    const direct = results["DBOBJ Direct (API)"][op].toFixed(2);
    const sql = results["DBOBJ SQL (Engine)"][op].toFixed(2);
    const prep = results["DBOBJ SQL (Prepared)"][op].toFixed(2);
    const sqlite = results["Bun SQLite (Native)"][op].toFixed(2);
    console.log(`${op.toUpperCase().padEnd(20)} | ${direct.padStart(10)}ms | ${sql.padStart(10)}ms | ${prep.padStart(10)}ms | ${sqlite.padStart(10)}ms`);
  }
  console.log("=".repeat(75));
}

runBenchmark();

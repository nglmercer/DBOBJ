import { TestSuite } from "./interface";
import { ROW_COUNT, UPDATE_COUNT } from "./constants";
import { DBOBJDirectSuite } from "./suites/direct";
import { DBOBJSchemaSuite } from "./suites/schema";
import { DBOBJSQLSuite } from "./suites/sql";
import { DBOBJSQLPreparedSuite } from "./suites/prepared";
import { BunSQLiteSuite } from "./suites/sqlite";

async function runBenchmark() {
  const suites: TestSuite[] = [
    new DBOBJDirectSuite(),
    new DBOBJSchemaSuite(),
    new DBOBJSQLSuite(),
    new DBOBJSQLPreparedSuite(),
    new BunSQLiteSuite()
  ];

  console.log(`\n--- BENCHMARK: ${ROW_COUNT} Rows ---`);

  const results: Record<string, Record<string, number>> = {};

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

  console.log("\n" + "=".repeat(88));
  console.log(`${"Operation".padEnd(20)} | ${"Direct".padEnd(12)} | ${"Schema".padEnd(12)} | ${"SQL Bulk".padEnd(12)} | ${"SQL Prep".padEnd(12)} | ${"Bun SQLite".padEnd(12)}`);
  console.log("-".repeat(88));

  const ops = ["insert", "read", "find", "update", "join"];
  for (const op of ops) {
    const direct = results["DBOBJ Direct (API)"][op].toFixed(2);
    const schema = results["DBOBJ Schema (Direct)"][op].toFixed(2);
    const sql = results["DBOBJ SQL (Engine)"][op].toFixed(2);
    const prep = results["DBOBJ SQL (Prepared)"][op].toFixed(2);
    const sqlite = results["Bun SQLite (Native)"][op].toFixed(2);
    console.log(`${op.toUpperCase().padEnd(20)} | ${direct.padStart(10)}ms | ${schema.padStart(10)}ms | ${sql.padStart(10)}ms | ${prep.padStart(10)}ms | ${sqlite.padStart(10)}ms`);
  }
  console.log("=".repeat(88));
}

runBenchmark();

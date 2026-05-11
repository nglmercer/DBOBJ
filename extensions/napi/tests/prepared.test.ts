import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("PreparedStatement run", () => {
  const db = new Database("Test_PrepRun");
  db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
  const stmt = db.prepare("INSERT INTO t (id, val) VALUES (?, ?)");
  stmt.run([1, 100]); stmt.run([2, 200]);
  const rows = db.getRows("t");
  expect(rows.length).toBe(2);
  expect(rows[0].val).toBe(100);
});

test("PreparedStatement allI64", () => {
  const db = new Database("Test_PrepAll");
  db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
  db.executeSql("INSERT INTO t (id, val) VALUES (1, 10), (2, 20)");
  const stmt = db.prepare("SELECT val FROM t");
  const col = stmt.allI64([]);
  expect(col.length).toBe(2);
  expect(col[0]).toBe(10n); expect(col[1]).toBe(20n);
});

test("PreparedStatement runBatch", () => {
  const db = new Database("Test_PrepBatch");
  db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
  const stmt = db.prepare("INSERT INTO t (id, val) VALUES (?, ?)");
  stmt.runBatch([[1, 10], [2, 20], [3, 30]]);
  expect(db.countRows("t")).toBe(3);
});

test("PreparedStatement runBatchI64", () => {
  const db = new Database("Test_PrepBatchI64");
  db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
  const stmt = db.prepare("INSERT INTO t (id, val) VALUES (?, ?)");
  stmt.runBatchI64(new BigInt64Array([BigInt(1), BigInt(100), BigInt(2), BigInt(200), BigInt(3), BigInt(300)]), 2);
  expect(db.getColumnI64("t", "val")[1]).toBe(200n);
});

test("PreparedStatement runBatchValues", () => {
  const db = new Database("Test_PrepBatchVal");
  db.executeSql("CREATE TABLE t (id INTEGER, name STRING)");
  const stmt = db.prepare("INSERT INTO t (id, name) VALUES (?, ?)");
  stmt.runBatchValues([1, "a", 2, "b"], 2);
  expect(db.getRows("t").length).toBe(2);
});

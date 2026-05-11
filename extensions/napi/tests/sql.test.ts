import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("SQL Execution", () => {
  const db = new Database("SQL_Test");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING)");
  db.executeSql("INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob')");
  const result = db.executeSql("SELECT * FROM users WHERE id = 1");
  expect(Array.isArray(result)).toBe(true);
  expect(result.length).toBe(1);
  expect(result[0].name).toBe("Alice");
});

test("queryI64", () => {
  const db = new Database("Test_QueryI64");
  db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
  db.executeSql("INSERT INTO t (id, val) VALUES (1, 10), (2, 20)");
  const col = db.queryI64("SELECT val FROM t");
  expect(col.length).toBe(2);
  expect(col[0]).toBe(10n);
  expect(col[1]).toBe(20n);
});

test("queryJoinI64 via hashJoinI64", () => {
  const db = new Database("Test_QueryJoin");
  db.createTable("t", [{ name: "val", dataType: DataType.Integer }]);
  db.createTable("t2", [{ name: "score", dataType: DataType.Integer }]);
  db.insertRowI64("t", [10]);
  db.insertRowI64("t2", [10]);
  const joined = db.hashJoinI64("t", "val", "t2", "score");
  expect(joined.length).toBe(2);
});

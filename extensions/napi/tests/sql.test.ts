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

// ── New SQL features ─────────────────────────────────────────────────

test("DROP TABLE", () => {
  const db = new Database("Test_SqlDrop");
  db.executeSql("CREATE TABLE t (id INTEGER)");
  db.executeSql("INSERT INTO t VALUES (1)");
  expect(db.countRows("t")).toBe(1);
  db.executeSql("DROP TABLE t");
  expect(() => db.getRows("t")).toThrow();
});

test("ORDER BY", () => {
  const db = new Database("Test_SqlOrd");
  db.executeSql("CREATE TABLE t (id INTEGER, name STRING)");
  db.executeSql("INSERT INTO t VALUES (2, 'Bob'), (1, 'Alice'), (3, 'Charlie')");
  const r = db.executeSql("SELECT * FROM t ORDER BY name");
  expect(r[0].name).toBe("Alice");
  expect(r[2].name).toBe("Charlie");
});

test("ORDER BY DESC", () => {
  const db = new Database("Test_SqlOrdDesc");
  db.executeSql("CREATE TABLE t (id INTEGER, name STRING)");
  db.executeSql("INSERT INTO t VALUES (1, 'Alice'), (2, 'Bob')");
  const r = db.executeSql("SELECT * FROM t ORDER BY name DESC");
  expect(r[0].name).toBe("Bob");
});

test("LIMIT", () => {
  const db = new Database("Test_SqlLim");
  db.executeSql("CREATE TABLE t (id INTEGER)");
  for (let i = 0; i < 10; i++) db.executeSql(`INSERT INTO t VALUES (${i})`);
  const r = db.executeSql("SELECT * FROM t ORDER BY id LIMIT 3");
  expect(r.length).toBe(3);
});

test("LIMIT with OFFSET", () => {
  const db = new Database("Test_SqlOff");
  db.executeSql("CREATE TABLE t (id INTEGER)");
  for (let i = 0; i < 10; i++) db.executeSql(`INSERT INTO t VALUES (${i})`);
  const r = db.executeSql("SELECT * FROM t ORDER BY id LIMIT 3 OFFSET 7");
  expect(r.length).toBe(3);
  expect(r[0].id).toBe(7);
  expect(r[2].id).toBe(9);
});

test("COUNT aggregation", () => {
  const db = new Database("Test_SqlCnt");
  db.executeSql("CREATE TABLE t (val INTEGER)");
  db.executeSql("INSERT INTO t VALUES (10), (20), (30)");
  const r = db.executeSql("SELECT COUNT(*) FROM t");
  expect(r[0]["COUNT(*)"]).toBe(3);
});

test("SUM aggregation", () => {
  const db = new Database("Test_SqlSum");
  db.executeSql("CREATE TABLE t (val INTEGER)");
  db.executeSql("INSERT INTO t VALUES (10), (20), (30)");
  const r = db.executeSql("SELECT SUM(val) FROM t");
  expect(r[0].SUM).toBe(60);
});

test("MIN / MAX aggregation", () => {
  const db = new Database("Test_SqlMinMax");
  db.executeSql("CREATE TABLE t (val INTEGER)");
  db.executeSql("INSERT INTO t VALUES (10), (20), (30)");
  expect(db.executeSql("SELECT MIN(val) FROM t")[0].MIN).toBe(10);
  expect(db.executeSql("SELECT MAX(val) FROM t")[0].MAX).toBe(30);
});

test("LIKE with ORDER BY", () => {
  const db = new Database("Test_SqlLikeOrd");
  db.executeSql("CREATE TABLE t (id INTEGER, name STRING)");
  db.executeSql("INSERT INTO t VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Alex')");
  const r = db.executeSql("SELECT * FROM t WHERE name LIKE 'A%' ORDER BY id");
  expect(r.length).toBe(2);
});

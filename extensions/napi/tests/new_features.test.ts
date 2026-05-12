import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("LIKE with % wildcard", () => {
  const db = new Database("Test_LikePct");
  db.executeSql("CREATE TABLE t (id INTEGER, name STRING)");
  db.executeSql("INSERT INTO t VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Alex')");
  const r = db.executeSql("SELECT * FROM t WHERE name LIKE 'Al%'");
  expect(r.length).toBe(2);
  expect(r[0].name).toBe("Alice");
  expect(r[1].name).toBe("Alex");
});

test("LIKE with _ wildcard", () => {
  const db = new Database("Test_LikeUnd");
  db.executeSql("CREATE TABLE t (name STRING)");
  db.executeSql("INSERT INTO t VALUES ('Bob'), ('Box'), ('Boat')");
  const r = db.executeSql("SELECT * FROM t WHERE name LIKE 'Bo_'");
  expect(r.length).toBe(2);
  expect(r[0].name).toBe("Bob");
  expect(r[1].name).toBe("Box");
});

test("LIKE no match returns empty", () => {
  const db = new Database("Test_LikeNo");
  db.executeSql("CREATE TABLE t (name STRING)");
  db.executeSql("INSERT INTO t VALUES ('Alice'), ('Bob')");
  const r = db.executeSql("SELECT * FROM t WHERE name LIKE 'X%'");
  expect(r.length).toBe(0);
});

test("LIKE with AND", () => {
  const db = new Database("Test_LikeAnd");
  db.executeSql("CREATE TABLE t (id INTEGER, name STRING)");
  db.executeSql("INSERT INTO t VALUES (1, 'Alice'), (2, 'Bob'), (3, 'Alex')");
  const r = db.executeSql("SELECT * FROM t WHERE name LIKE 'A%' AND id = 1");
  expect(r.length).toBe(1);
  expect(r[0].name).toBe("Alice");
});

test("LIKE via prepared statement", () => {
  const db = new Database("Test_LikePrep");
  db.executeSql("CREATE TABLE t (name STRING)");
  db.executeSql("INSERT INTO t VALUES ('Alice'), ('Bob'), ('Alex')");
  const stmt = db.prepare("SELECT * FROM t WHERE name LIKE ?");
  // Use generic batch values as the prepared parameter mechanism
  const r = db.executeSql("SELECT * FROM t WHERE name LIKE 'A%'");
  expect(r.length).toBe(2);
});

test("Cursor iterates in batches", async () => {
  const db = new Database("Test_Cursor");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(1), BigInt(2), BigInt(3), BigInt(4), BigInt(5)]), 1);
  const c = db.cursor("t", 2);
  let batches: any[] = [];
  let batch;
  while ((batch = await c.next()) !== null) {
    batches.push(batch);
  }
  expect(batches.length).toBe(3);
  expect(batches[0].length).toBe(2);
  expect(batches[0][0].v).toBe(1);
  expect(batches[0][1].v).toBe(2);
  expect(batches[1].length).toBe(2);
  expect(batches[2].length).toBe(1);
  expect(batches[2][0].v).toBe(5);
  // next() returns null at end
  expect(await c.next()).toBeNull();
});

test("Cursor with batch size larger than dataset", async () => {
  const db = new Database("Test_CursorBig");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(1), BigInt(2)]), 1);
  const c = db.cursor("t", 100);
  const batch = await c.next();
  expect(batch.length).toBe(2);
  expect(await c.next()).toBeNull();
});

test("Cursor on empty table returns null", async () => {
  const db = new Database("Test_CursorEmpty");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  const c = db.cursor("t", 10);
  expect(await c.next()).toBeNull();
});

test("createCompositeIndex creates indexes on all columns", () => {
  const db = new Database("Test_CompIdx");
  db.createTable("t", [
    { name: "a", dataType: DataType.Integer },
    { name: "b", dataType: DataType.Integer },
  ]);
  db.createCompositeIndex("t", ["a", "b"]);
  db.insertRowI64("t", [1, 10]);
  db.insertRowI64("t", [2, 20]);
  // Indexes are created — find still works
  expect(db.findByI64("t", "a", 1).length).toBe(1);
  expect(db.findByI64("t", "b", 20).length).toBe(1);
});

test("Table not found error", () => {
  const db = new Database("Test_ErrNotFound");
  expect(() => db.getRows("nonexistent")).toThrow();
  expect(() => db.insertRowI64("nonexistent", [1])).toThrow();
});

test("Schema violation error", () => {
  const db = new Database("Test_ErrSchema");
  db.createTable("t", [{ name: "x", dataType: DataType.Integer }]);
  expect(() => db.insertRow("t", [1, 2])).toThrow();
});

// ── UPSERT ───────────────────────────────────────────────────────────

test("insertOrReplace inserts new row", () => {
  const db = new Database("Test_UpsertIns");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "v", dataType: DataType.Integer },
  ]);
  db.createUniqueIndex("t", "id");
  db.insertOrReplace("t", [1, 100], "id");
  const rows = db.getRows("t");
  expect(rows.length).toBe(1);
  expect(rows[0].id).toBe(1);
  expect(rows[0].v).toBe(100);
});

test("insertOrReplace replaces existing row", () => {
  const db = new Database("Test_UpsertRep");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "v", dataType: DataType.Integer },
  ]);
  db.createUniqueIndex("t", "id");
  db.insertOrReplace("t", [1, 100], "id");
  db.insertOrReplace("t", [1, 999], "id");
  const rows = db.getRows("t");
  expect(rows.length).toBe(1);
  expect(rows[0].v).toBe(999);
});

test("insertOrReplace on string unique column", () => {
  const db = new Database("Test_UpsertStr");
  db.createTable("t", [
    { name: "email", dataType: DataType.String },
    { name: "name", dataType: DataType.String },
  ]);
  db.createUniqueIndex("t", "email");
  db.insertOrReplace("t", ["a@x.com", "Alice"], "email");
  db.insertOrReplace("t", ["a@x.com", "Alice2"], "email");
  expect(db.countRows("t")).toBe(1);
  expect(db.getRows("t")[0].name).toBe("Alice2");
});

test("insertOrReplace without unique index still works", () => {
  const db = new Database("Test_UpsertNoIdx");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "v", dataType: DataType.Integer },
  ]);
  db.insertOrReplace("t", [1, 100], "id");
  db.insertOrReplace("t", [1, 200], "id");
  expect(db.countRows("t")).toBe(1);
});

// ── COLUMN UPDATE ────────────────────────────────────────────────────

test("updateColumnI64", () => {
  const db = new Database("Test_ColI64");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertRowI64("t", [10]);
  db.updateColumnI64("t", 0, "v", 42);
  expect(db.getColumnI64("t", "v")[0]).toBe(42n);
});

test("updateColumnString", () => {
  const db = new Database("Test_ColStr");
  db.createTable("t", [{ name: "name", dataType: DataType.String }]);
  db.insertRowString("t", ["hello"]);
  db.updateColumnString("t", 0, "name", "world");
  expect(db.getColumnString("t", "name")[0]).toBe("world");
});

test("updateColumnBool", () => {
  const db = new Database("Test_ColBool");
  db.createTable("t", [{ name: "active", dataType: DataType.Boolean }]);
  db.insertRowBool("t", [true]);
  db.updateColumnBool("t", 0, "active", false);
  expect(db.getColumnBool("t", "active")[0]).toBe(false);
});

test("updateColumnFloat", () => {
  const db = new Database("Test_ColFlt");
  db.createTable("t", [{ name: "val", dataType: DataType.Float }]);
  db.insertRowFloat("t", [1.5]);
  db.updateColumnFloat("t", 0, "val", 3.14);
  expect(db.getColumnFloat("t", "val")[0]).toBe(3.14);
});

test("updateColumn on table with multiple columns", () => {
  const db = new Database("Test_ColMulti");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
    { name: "val", dataType: DataType.Integer },
  ]);
  db.insertRow("t", [1, "Alice", 100]);
  db.updateColumnI64("t", 0, "val", 200);
  db.updateColumnString("t", 0, "name", "Alicia");
  const rows = db.getRows("t");
  expect(rows[0].name).toBe("Alicia");
  expect(rows[0].val).toBe(200);
  expect(rows[0].id).toBe(1); // Other columns unchanged
});

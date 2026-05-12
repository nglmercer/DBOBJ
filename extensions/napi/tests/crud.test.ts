import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("CRUD Operations", () => {
  const db = new Database("CRUD_Test");
  db.createTable("users", [{ name: "age", dataType: DataType.Integer }]);
  db.insertRowI64("users", [25]);
  db.insertRowI64("users", [30]);
  let ages = db.getColumnI64("users", "age");
  expect(ages.length).toBe(2);
  expect(ages[0]).toBe(25n);
  expect(ages[1]).toBe(30n);
  db.updateRowI64("users", 0, [35]);
  ages = db.getColumnI64("users", "age");
  expect(ages[0]).toBe(35n);
  const found = db.findByI64("users", "age", 35);
  expect(found.length).toBe(1);
  expect(found[0]).toBe(0n);
  db.deleteRow("users", 0);
  ages = db.getColumnI64("users", "age");
  expect(ages.length).toBe(1);
  expect(ages[0]).toBe(30n);
});

test("Generic insertRow with mixed types", () => {
  const db = new Database("Test_GenInsert");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
    { name: "active", dataType: DataType.Boolean },
  ]);
  db.insertRow("t", [1, "alice", true]);
  db.insertRow("t", [2, "bob", false]);
  const rows = db.getRows("t");
  expect(rows.length).toBe(2);
  expect(rows[0].id).toBe(1);
  expect(rows[0].name).toBe("alice");
  expect(rows[0].active).toBe(true);
  expect(rows[1].id).toBe(2);
  expect(rows[1].name).toBe("bob");
  expect(rows[1].active).toBe(false);
});

test("Generic updateRow with mixed types", () => {
  const db = new Database("Test_GenUpdate");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
    { name: "val", dataType: DataType.Integer },
  ]);
  db.insertRow("t", [1, "alice", 100]);
  db.updateRow("t", 0, [1, "alice_updated", 200]);
  const rows = db.getRows("t");
  expect(rows.length).toBe(1);
  expect(rows[0].name).toBe("alice_updated");
  expect(rows[0].val).toBe(200);
});

test("Generic insertBatch with mixed types", () => {
  const db = new Database("Test_GenBatch");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);
  db.insertBatch("t", [1, "a", 2, "b", 3, "c"], 2);
  const rows = db.getRows("t");
  expect(rows.length).toBe(3);
  expect(rows[0].id).toBe(1);
  expect(rows[0].name).toBe("a");
  expect(rows[2].name).toBe("c");
});

// ── GET ROW BY ID / COLUMN ──────────────────────────────────────────

test("getRowById returns correct row", () => {
  const db = new Database("Test_RowById");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);
  db.insertRow("t", [1, "Alice"]);
  db.insertRow("t", [2, "Bob"]);
  const row = db.getRowById("t", 0);
  expect(row).not.toBeNull();
  expect(row.id).toBe(1);
  expect(row.name).toBe("Alice");
  expect(db.getRowById("t", 99)).toBeNull();
});

test("getRowByColumnI64", () => {
  const db = new Database("Test_RowColI64");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);
  db.insertRow("t", [1, "Alice"]);
  db.insertRow("t", [2, "Bob"]);
  const row = db.getRowByColumnI64("t", "id", 1);
  expect(row).not.toBeNull();
  expect(row.name).toBe("Alice");
  expect(db.getRowByColumnI64("t", "id", 99)).toBeNull();
});

test("getRowByColumnString", () => {
  const db = new Database("Test_RowColStr");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "email", dataType: DataType.String },
  ]);
  db.insertRow("t", [1, "a@x.com"]);
  db.insertRow("t", [2, "b@x.com"]);
  const row = db.getRowByColumnString("t", "email", "b@x.com");
  expect(row).not.toBeNull();
  expect(row.id).toBe(2);
  expect(db.getRowByColumnString("t", "email", "nope")).toBeNull();
});

test("getRowByColumnBool", () => {
  const db = new Database("Test_RowColBool");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "active", dataType: DataType.Boolean },
  ]);
  db.insertRow("t", [1, true]);
  db.insertRow("t", [2, false]);
  const row = db.getRowByColumnBool("t", "active", true);
  expect(row).not.toBeNull();
  expect(row.id).toBe(1);
});

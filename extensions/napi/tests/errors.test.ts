import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("Schema violation error shows column list", () => {
  const db = new Database("Test_ErrMsg");
  db.createTable("users", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);
  try {
    db.insertRow("users", [1]);
    expect.unreachable();
  } catch (e: any) {
    expect(e.message).toContain("users");
    expect(e.message).toContain("id (Integer)");
    expect(e.message).toContain("name (String)");
    expect(e.message).toContain("insert_batch_values");
  }
});

test("Table not found error", () => {
  const db = new Database("Test_NoTable");
  expect(() => db.getRows("nonexistent")).toThrow();
  expect(() => db.insertRowI64("nonexistent", [1])).toThrow();
  expect(() => db.deleteRow("nonexistent", 0)).toThrow();
});

test("Table not found on insert throws", () => {
  const db = new Database("Test_NoTableInsert");
  expect(() => db.insertRow("nonexistent", [1])).toThrow();
});

test("Float column via insertRow", () => {
  const db = new Database("Test_Float");
  db.createTable("t", [{ name: "val", dataType: DataType.Float }]);
  db.insertRow("t", [3.14]);
  expect(db.getRows("t").length).toBe(1);
  expect(typeof db.getRows("t")[0].val).toBe("number");
});

test("Nullable columns accept null", () => {
  const db = new Database("Test_Nullable");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer, nullable: false },
    { name: "name", dataType: DataType.String },
  ]);
  db.insertRow("t", [1, null]);
  expect(db.getRows("t")[0].name).toBeNull();
});

test("Nullable columns accept undefined", () => {
  const db = new Database("Test_Undef");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);
  db.insertRow("t", [1, undefined]);
  expect(db.getColumnString("t", "name")[0]).toBe("");
});

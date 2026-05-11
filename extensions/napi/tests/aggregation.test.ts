import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("sumColumn", () => {
  const db = new Database("Test_Sum");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(10), BigInt(20), BigInt(30)]), 1);
  expect(db.sumColumn("t", "v")).toBe(60);
});

test("minColumn", () => {
  const db = new Database("Test_Min");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(5), BigInt(3), BigInt(8)]), 1);
  expect(db.minColumn("t", "v")).toBe(3);
});

test("maxColumn", () => {
  const db = new Database("Test_Max");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(5), BigInt(3), BigInt(8)]), 1);
  expect(db.maxColumn("t", "v")).toBe(8);
});

test("avgColumn", () => {
  const db = new Database("Test_Avg");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(10), BigInt(20), BigInt(30)]), 1);
  expect(db.avgColumn("t", "v")).toBe(20);
});

test("schema.validateRow passes valid data", () => {
  const db = new Database("Test_ValOk");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
    { name: "active", dataType: DataType.Boolean },
  ]);
  expect(db.schema.validateRow("t", [1, "alice", true])).toEqual([]);
});

test("schema.validateRow catches type errors", () => {
  const db = new Database("Test_ValErr");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);
  const errors = db.schema.validateRow("t", ["x", 42]);
  expect(errors.length).toBe(2);
  expect(errors[0]).toContain("id");
  expect(errors[1]).toContain("name");
});

test("schema.validateRow catches nullable violation", () => {
  const db = new Database("Test_ValNull");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer, nullable: false },
  ]);
  const errors = db.schema.validateRow("t", [null]);
  expect(errors.length).toBe(1);
  expect(errors[0]).toContain("expected Integer, got Null");
});

test("schema.validateRow catches count mismatch", () => {
  const db = new Database("Test_ValCount");
  db.createTable("t", [{ name: "a", dataType: DataType.Integer }]);
  const errors = db.schema.validateRow("t", [1, 2]);
  expect(errors.length).toBe(1);
  expect(errors[0]).toContain("expected 1 values, got 2");
});

test("getRowsAsync returns same data as getRows", async () => {
  const db = new Database("Test_Async");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(1), BigInt(2)]), 1);
  const sync = db.getRows("t");
  const async = await db.getRowsAsync("t");
  expect(Array.isArray(async)).toBe(true);
  expect(async.length).toBe(sync.length);
  expect(async[0].v).toBe(sync[0].v);
});

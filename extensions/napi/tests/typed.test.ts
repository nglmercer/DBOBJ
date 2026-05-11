import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("Typed Insert/Update methods", () => {
  const db = new Database("Typed_Test");
  db.createTable("strings", [{ name: "val", dataType: DataType.String }]);
  db.insertRowString("strings", ["hello"]);
  db.insertRowString("strings", ["world"]);
  db.updateRowString("strings", 0, ["hi"]);
  let rows = db.getRows("strings");
  expect(rows.length).toBe(2);
  expect(rows[0].val).toBe("hi");
  expect(rows[1].val).toBe("world");

  db.createTable("bools", [{ name: "val", dataType: DataType.Boolean }]);
  db.insertRowBool("bools", [true]);
  db.insertRowBool("bools", [false]);
  db.updateRowBool("bools", 0, [false]);
  rows = db.getRows("bools");
  expect(rows.length).toBe(2);
  expect(rows[0].val).toBe(false);
  expect(rows[1].val).toBe(false);

  db.createTable("batch_str", [
    { name: "a", dataType: DataType.String },
    { name: "b", dataType: DataType.String },
  ]);
  db.insertBatchString("batch_str", ["x", "y", "z", "w"], 2);
  rows = db.getRows("batch_str");
  expect(rows.length).toBe(2);
  expect(rows[0].a).toBe("x");
  expect(rows[1].b).toBe("w");

  db.createTable("batch_bool", [
    { name: "a", dataType: DataType.Boolean },
    { name: "b", dataType: DataType.Boolean },
  ]);
  db.insertBatchBool("batch_bool", [true, false, false, true], 2);
  rows = db.getRows("batch_bool");
  expect(rows.length).toBe(2);
  expect(rows[0].a).toBe(true);
  expect(rows[1].b).toBe(true);
});

test("insertBatchI64", () => {
  const db = new Database("Test_BatchI64");
  db.createTable("t", [
    { name: "a", dataType: DataType.Integer },
    { name: "b", dataType: DataType.Integer },
  ]);
  const batch = new BigInt64Array([BigInt(10), BigInt(100), BigInt(20), BigInt(200)]);
  db.insertBatchI64("t", batch, 2);
  const rows = db.getRows("t");
  expect(rows.length).toBe(2);
  expect(rows[0].a).toBe(10);
  expect(rows[0].b).toBe(100);
  expect(rows[1].a).toBe(20);
  expect(rows[1].b).toBe(200);
});

test("Float typed methods", () => {
  const db = new Database("Test_FloatTyped");
  db.createTable("t", [{ name: "val", dataType: DataType.Float }]);
  db.insertRowFloat("t", [3.14]);
  db.insertBatchFloat("t", [1.5, 2.5], 1);
  expect(db.countRows("t")).toBe(3);
  const col = db.getColumnFloat("t", "val");
  expect(col.length).toBe(3);
  expect(col[0]).toBe(3.14);
  expect(col[1]).toBe(1.5);
  expect(col[2]).toBe(2.5);
  db.updateRowFloat("t", 0, [2.71]);
  expect(db.getColumnFloat("t", "val")[0]).toBe(2.71);
});

test("getColumnString / getColumnBool / getColumnFloat", () => {
  const db = new Database("Test_ColStrBoolFlt");
  db.createTable("s", [{ name: "v", dataType: DataType.String }]);
  db.createTable("b", [{ name: "v", dataType: DataType.Boolean }]);
  db.createTable("f", [{ name: "v", dataType: DataType.Float }]);
  db.insertRowString("s", ["a"]); db.insertRowString("s", ["b"]);
  db.insertRowBool("b", [true]); db.insertRowBool("b", [false]);
  db.insertRowFloat("f", [1.1]); db.insertRowFloat("f", [2.2]);
  expect(db.getColumnString("s", "v").length).toBe(2);
  expect(db.getColumnBool("b", "v").length).toBe(2);
  expect(db.getColumnFloat("f", "v").length).toBe(2);
});

test("findByString / findByBool", () => {
  const db = new Database("Test_FindStrBool");
  db.createTable("s", [{ name: "v", dataType: DataType.String }]);
  db.createTable("b", [{ name: "v", dataType: DataType.Boolean }]);
  db.insertRowString("s", ["alpha"]); db.insertRowString("s", ["beta"]); db.insertRowString("s", ["alpha"]);
  db.insertRowBool("b", [true]); db.insertRowBool("b", [false]); db.insertRowBool("b", [true]);
  expect(db.findByString("s", "v", "alpha").length).toBe(2);
  expect(db.findByBool("b", "v", true).length).toBe(2);
});

test("updateBatchI64", () => {
  const db = new Database("Test_UpdBatch");
  db.createTable("t", [{ name: "val", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(10), BigInt(20), BigInt(30)]), 1);
  db.updateBatchI64("t", "val", new BigInt64Array([BigInt(99), BigInt(0), BigInt(88), BigInt(2)]));
  const vals = db.getColumnI64("t", "val");
  expect(vals[0]).toBe(99n);
  expect(vals[1]).toBe(20n);
  expect(vals[2]).toBe(88n);
});

test("deleteBatchI64", () => {
  const db = new Database("Test_DelBatch");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertBatchI64("t", new BigInt64Array([BigInt(1), BigInt(2), BigInt(3)]), 1);
  expect(db.deleteBatchI64("t", new BigInt64Array([BigInt(0), BigInt(2)]))).toBe(2);
  expect(db.countRows("t")).toBe(1);
  expect(db.getColumnI64("t", "v")[0]).toBe(2n);
});

test("deleteByColumn", () => {
  const db = new Database("Test_DelByCol");
  db.createTable("t", [
    { name: "v", dataType: DataType.Integer },
    { name: "s", dataType: DataType.String },
  ]);
  db.insertRow("t", [1, "a"]); db.insertRow("t", [2, "b"]); db.insertRow("t", [1, "c"]);
  expect(db.deleteByColumnI64("t", "v", 1)).toBe(2);
  expect(db.countRows("t")).toBe(1);
});

test("countRows", () => {
  const db = new Database("Test_Count");
  db.createTable("t", [{ name: "x", dataType: DataType.Integer }]);
  expect(db.countRows("t")).toBe(0);
  db.insertRowI64("t", [1]); db.insertRowI64("t", [2]); db.insertRowI64("t", [3]);
  expect(db.countRows("t")).toBe(3);
});

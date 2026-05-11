import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("createIndex / createUniqueIndex", () => {
  const db = new Database("Test_Index");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer, nullable: false },
    { name: "val", dataType: DataType.Integer },
  ]);
  db.createIndex("t", "val");
  db.createUniqueIndex("t", "id");
  db.insertRowI64("t", [1, 10]);
  db.insertRowI64("t", [2, 20]);
});

test("getTableMetadata", () => {
  const db = new Database("Test_Meta");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);
  let meta = db.getTableMetadata("t");
  expect(meta).not.toBeNull();
  expect(meta!.name).toBe("t");
  expect(meta!.columnCount).toBe(2);
  expect(meta!.rowCount).toBe(0);
  db.insertRow("t", [1, "a"]);
  expect(db.getTableMetadata("t")!.rowCount).toBe(1);
});

test("listTables", () => {
  const db = new Database("Test_List");
  db.createTable("a", [{ name: "x", dataType: DataType.Integer }]);
  db.createTable("b", [{ name: "y", dataType: DataType.Integer }]);
  const tables = db.listTables();
  expect(tables.length).toBe(2);
  expect(tables).toContain("a"); expect(tables).toContain("b");
});

test("save / load roundtrip", () => {
  const path = `Test_Save_${Date.now()}.dbobj`;
  const db = new Database(path);
  db.createTable("t", [{ name: "val", dataType: DataType.Integer }]);
  db.insertRowI64("t", [42]);
  db.save(path);
  const loaded = Database.load(path);
  expect(loaded.getTableMetadata("t")!.rowCount).toBe(1);
  expect(loaded.getColumnI64("t", "val")[0]).toBe(42n);
});

test("Hash Join", () => {
  const db = new Database("Join_Test");
  db.createTable("t1", [{ name: "val", dataType: DataType.Integer }]);
  db.createTable("t2", [{ name: "val", dataType: DataType.Integer }]);
  db.insertRowI64("t1", [10]); db.insertRowI64("t2", [10]);
  const joinResult = db.hashJoinI64("t1", "val", "t2", "val");
  expect(joinResult.length).toBe(2);
  expect(joinResult[0]).toBe(0n); expect(joinResult[1]).toBe(0n);
});

import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("Transaction rollback reverts changes", () => {
  const db = new Database("Test_TxRoll");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertRowI64("t", [1]); db.insertRowI64("t", [2]);
  const tx = db.beginTransaction();
  db.insertRowI64("t", [3]);
  expect(db.countRows("t")).toBe(3);
  tx.rollback();
  expect(db.countRows("t")).toBe(2);
});

test("Transaction commit persists changes", () => {
  const db = new Database("Test_TxCom");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertRowI64("t", [1]);
  const tx = db.beginTransaction();
  db.insertRowI64("t", [2]);
  tx.commit();
  expect(db.countRows("t")).toBe(2);
});

test("Transaction rollback with multiple changes", () => {
  const db = new Database("Test_TxMulti");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertRowI64("t", [1]);
  const tx = db.beginTransaction();
  db.updateRowI64("t", 0, [99]);
  db.deleteRow("t", 0);
  expect(db.countRows("t")).toBe(0);
  tx.rollback();
  expect(db.countRows("t")).toBe(1);
  expect(db.getColumnI64("t", "v")[0]).toBe(1n);
});

test("countRows after transaction rollback", () => {
  const db = new Database("Test_TxCount");
  db.createTable("t", [{ name: "v", dataType: DataType.Integer }]);
  db.insertRowI64("t", [1]); db.insertRowI64("t", [2]);
  const tx = db.beginTransaction();
  db.insertRowI64("t", [3]); db.insertRowI64("t", [4]);
  expect(db.countRows("t")).toBe(4);
  tx.rollback();
  expect(db.countRows("t")).toBe(2);
});

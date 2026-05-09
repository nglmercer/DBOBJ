import { expect, test, describe } from "bun:test";
const { Database } = require("./index.node") as typeof import("./index.d.ts");

describe("DBOBJ N-API Bindings", () => {
  test("should create a table and insert data", () => {
    const db = new Database("TestDB");
    db.createTable("test", ["id", "val"], ["integer", "integer"]);
    db.insertRowI64("test", [1, 100]);
    
    const col = db.getColumnI64("test", "val");
    
    expect(col.length).toBe(1);
    expect(col[0]).toBe(100n);
  });

  test("zero-copy buffer should be a BigInt64Array", () => {
    const db = new Database("TypeDB");
    db.createTable("test", ["val"], ["integer"]);
    db.insertRowI64("test", [42]);
    
    const col = db.getColumnI64("test", "val");
    
    expect(col).toBeInstanceOf(BigInt64Array);
  });
});

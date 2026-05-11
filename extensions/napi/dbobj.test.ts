import { expect, test, describe } from "bun:test";
const { Database } = require("./index.js") as typeof import("./index.d.ts");

describe("DBOBJ N-API Bindings - Full Operations", () => {
  test("CRUD Operations", () => {
    const db = new Database("CRUD_Test");
    db.createTable("users", ["age"], ["integer"]);

    // Insert
    db.insertRowI64("users", [25]);
    db.insertRowI64("users", [30]);

    let ages = db.getColumnI64("users", "age");
    expect(ages.length).toBe(2);
    expect(ages[0]).toBe(25n);
    expect(ages[1]).toBe(30n);

    // Update (ID 0 is the first row)
    db.updateRowI64("users", 0, [35]);
    ages = db.getColumnI64("users", "age");
    expect(ages[0]).toBe(35n);

    // Find
    const found = db.findByI64("users", "age", 35);
    expect(found.length).toBe(1);
    expect(found[0]).toBe(0n); // ID of the first row

    // Delete
    db.deleteRow("users", 0);
    ages = db.getColumnI64("users", "age");
    expect(ages.length).toBe(1);
    expect(ages[0]).toBe(30n);
  });

  test("Hash Join", () => {
    const db = new Database("Join_Test");
    db.createTable("t1", [
      { name: "val", dataType: DataType.Integer },
    ]);
    db.createTable("t2", [
      { name: "val", dataType: DataType.Integer },
    ]);

    db.insertRowI64("t1", [10]); // ID 0
    db.insertRowI64("t2", [10]); // ID 0

    const joinResult = db.hashJoinI64("t1", "val", "t2", "val");
    expect(joinResult.length).toBe(2); // Pair of IDs: [0, 0]
    expect(joinResult[0]).toBe(0n);
    expect(joinResult[1]).toBe(0n);
  });

  test("SQL Execution", () => {
    const db = new Database("SQL_Test");
    db.executeSql("CREATE TABLE users (id INTEGER, name STRING)");
    db.executeSql("INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob')");

    const result = db.executeSql("SELECT * FROM users WHERE id = 1");
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBe(1);
    expect(result[0].name).toBe("Alice");
  });
});

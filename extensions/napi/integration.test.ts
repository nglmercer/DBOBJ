import { expect, test, describe, beforeAll } from "bun:test";
import { Database, DynamicSchema, DataType } from "./index";
import { unlinkSync, existsSync } from "node:fs";

describe("Database DynamicSchema Integration", () => {
  const dbName = "integration.dbobj";
  const schemaName = "user_schema";
  const tableName = "users";

  beforeAll(() => {
    if (existsSync(dbName)) {
      unlinkSync(dbName);
    }
  });

  test("create table and insert/update object", () => {
    const db = new Database(dbName);
    const ds = new DynamicSchema();
    ds.register(schemaName, [
      { name: "id", type: DataType.Integer },
      { name: "name", type: DataType.String },
      { name: "age", type: DataType.Integer, optional: true },
    ]);

    db.createTableFromSchema(tableName, ds, schemaName);

    const user1 = { id: 1, name: "Alice", age: 30 };
    db.insertObject(tableName, user1, ds, schemaName);

    const user2 = { id: 2, name: "Bob" };
    db.insertObject(tableName, user2, ds, schemaName);

    const rows = db.getRows(tableName);
    expect(rows).toHaveLength(2);
    expect(rows[0]).toEqual({ id: 1, name: "Alice", age: 30 });
    expect(rows[1]).toEqual({ id: 2, name: "Bob", age: null });

    const user1Update = { id: 1, name: "Alice Smith", age: 31 };
    db.updateObject(tableName, 0, user1Update, ds, schemaName);

    const updatedRow = db.getRowById(tableName, 0);
    expect(updatedRow).toEqual({ id: 1, name: "Alice Smith", age: 31 });
  });

  test("handle JSON objects and non-finite numbers", () => {
    const db = new Database(":memory:");
    const ds = new DynamicSchema();
    ds.register("complex", [
      { name: "id", type: DataType.Integer },
      { name: "data", type: DataType.Json },
      { name: "val", type: DataType.Float },
    ]);
    db.createTableFromSchema("complex_table", ds, "complex");

    const row = { id: 1, data: { nested: "value", num: 123 }, val: NaN };
    db.insertObject("complex_table", row, ds, "complex");

    const inserted = db.getRowById("complex_table", 0);
    expect(inserted.data).toEqual({ nested: "value", num: 123 });
    expect(inserted.val).toBeNull(); // NaN becomes null in our implementation
  });

  test("insert batch objects", () => {
    const db = new Database(":memory:");
    const ds = new DynamicSchema();
    ds.register(schemaName, [
      { name: "id", type: DataType.Integer },
      { name: "name", type: DataType.String },
    ]);
    db.createTableFromSchema(tableName, ds, schemaName);

    const users = [
      { id: 1, name: "User 1" },
      { id: 2, name: "User 2" },
      { id: 3, name: "User 3" },
    ];
    db.insertBatchObjects(tableName, users, ds, schemaName);

    expect(db.countRows(tableName)).toBe(3);
  });
});

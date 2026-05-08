import { describe, test, expect, beforeAll, afterAll } from "bun:test";
import {
  open,
  close,
  execute,
  createTable,
  insert,
  insertBatch,
  insertObject,
  select,
  selectAll,
  update,
  deleteRow,
  createIndex,
  listTables,
  tableInfo,
  save,
  load,
  type DatabaseHandle,
} from "./binding";

let handle: DatabaseHandle;

beforeAll(() => {
  handle = open("test_db");
});

afterAll(() => {
  close(handle);
});

describe("DBOBJ FFI", () => {
  test("open returns a numeric handle", () => {
    expect(typeof handle).toBe("number");
    expect(handle).toBeGreaterThan(0);
  });

  test("create table via SQL", () => {
    execute(handle, "CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
    const tables = listTables(handle);
    expect(tables).toContain("users");
  });

  test("create table via FFI with columns", () => {
    createTable(handle, "products", [
      { name: "id", type: "integer" },
      { name: "name", type: "string" },
      { name: "price", type: "float" },
    ]);
    const tables = listTables(handle);
    expect(tables).toContain("products");
    expect(tables.length).toBe(2);
  });

  test("insert values via FFI returns auto-generated ID", () => {
    const id1 = insert(handle, "products", [1, "Widget", 9.99]);
    const id2 = insert(handle, "products", [2, "Gadget", 19.99]);
    // Auto-generated IDs are sequential strings starting from "0"
    expect(id1).toBe("0");
    expect(id2).toBe("1");
  });

  test("insert object via FFI", () => {
    const id = insertObject(handle, "products", {
      id: 3,
      name: "Thingamajig",
      price: 29.99,
    });
    expect(id).toBe("2");
  });

  test("insert batch via FFI", () => {
    const ids = insertBatch(handle, "products", [
      [4, "Doohickey", 39.99],
      [5, "Contraption", 49.99],
    ]);
    expect(ids).toBeArray();
    expect(ids).toEqual(["3", "4"]);
  });

  test("insert via SQL", () => {
    execute(handle, "INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)");
    execute(handle, "INSERT INTO users (id, name, age) VALUES (2, 'Bob', 25)");
  });

  test("select all rows from products", () => {
    const rows = selectAll(handle, "products");
    expect(rows).toBeArray();
    expect(rows.length).toBe(5);
  });

  test("select all from users", () => {
    const rows = selectAll(handle, "users");
    expect(rows).toBeArray();
    expect(rows.length).toBe(2);
  });

  test("select with column filter by value", () => {
    const rows = select(handle, "products", "name", "Widget");
    expect(rows).toBeArray();
    expect(rows.length).toBe(1);
    expect(rows[0].name).toBe("Widget");
    expect(rows[0].price).toBe(9.99);
  });

  test("SQL select with WHERE", () => {
    const rows = execute(handle, "SELECT id, name, age FROM users WHERE age > 25");
    expect(rows).toBeArray();
    expect(rows.length).toBe(1);
    expect(rows[0].name).toBe("Alice");
  });

  test("update by row ID", () => {
    // Row "0" is the Widget row (first insert)
    update(handle, "products", "0", [1, "SuperWidget", 14.99]);
    const rows = selectAll(handle, "products");
    const widget = rows.find((r: any) => r.name === "SuperWidget");
    expect(widget).toBeDefined();
    expect(widget.price).toBe(14.99);
  });

  test("SQL update", () => {
    execute(handle, "UPDATE users SET age = 31 WHERE id = 1");
    const rows = execute(handle, "SELECT age FROM users WHERE id = 1");
    expect(rows[0].age).toBe(31);
  });

  test("delete by row ID", () => {
    deleteRow(handle, "products", "4");
    const rows = selectAll(handle, "products");
    expect(rows.length).toBe(4);
  });

  test("create index", () => {
    createIndex(handle, "products", "name");
    const rows = select(handle, "products", "name", "Gadget");
    expect(rows.length).toBe(1);
  });

  test("create unique index", () => {
    createIndex(handle, "products", "id", true);
  });

  test("table info", () => {
    const info = tableInfo(handle, "products");
    expect(info.name).toBe("products");
    expect(info.columns).toBeArray();
    expect(info.columns.length).toBe(3);
    expect(info.row_count).toBe(4);
  });

  test("save and load persistence", () => {
    const savePath = "/tmp/dbobj_test_persist.db";
    save(handle, savePath);

    const handle2 = load(savePath);
    const rows = selectAll(handle2, "products");
    expect(rows.length).toBe(4);
    const tables = listTables(handle2);
    expect(tables).toContain("users");
    expect(tables).toContain("products");

    close(handle2);
  });
});

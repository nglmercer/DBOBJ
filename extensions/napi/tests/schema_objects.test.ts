import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("objectsToArrowIpc produces valid IPC buffer for insertFromArrow", () => {
  const db = new Database("SchemaObjects1");
  db.createTable("users", [
    { name: "id", dataType: DataType.Integer },
    { name: "score", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);

  const objects = [
    { id: 0, score: 100, name: "Alice" },
    { id: 1, score: 200, name: "Bob" },
    { id: 2, score: 300, name: "Charlie" },
  ];

  const buf = db.objectsToArrowIpc("users", objects);
  expect(buf instanceof Buffer).toBe(true);
  expect(buf.length).toBeGreaterThan(0);

  const qb = db.createQueryBuilder();
  const n = qb.insertFromArrow("users", buf);
  expect(n).toBe(3);

  const rows = qb.select("users").execute() as Array<any>;
  expect(rows.length).toBe(3);
  expect(rows[0].name).toBe("Alice");
  expect(rows[1].score).toBe(200);
  expect(rows[2].id).toBe(2);
});

test("objectsToArrowIpc with mixed types", () => {
  const db = new Database("SchemaObjects2");
  db.createTable("items", [
    { name: "id", dataType: DataType.Integer },
    { name: "price", dataType: DataType.Float },
    { name: "active", dataType: DataType.Boolean },
    { name: "name", dataType: DataType.String },
  ]);

  const buf = db.objectsToArrowIpc("items", [
    { id: 0, price: 9.99, active: true, name: "Widget" },
    { id: 1, price: 19.99, active: false, name: "Gadget" },
  ]);

  const qb = db.createQueryBuilder();
  const n = qb.insertFromArrow("items", buf);
  expect(n).toBe(2);

  const rows = qb.select("items").execute() as Array<any>;
  expect(rows[0].price).toBeCloseTo(9.99);
  expect(rows[0].active).toBe(true);
  expect(rows[1].active).toBe(false);
});

test("objectsToArrowIpc returns empty buffer for empty objects", () => {
  const db = new Database("SchemaObjects3");
  db.createTable("t", [{ name: "id", dataType: DataType.Integer }]);
  const buf = db.objectsToArrowIpc("t", []);
  expect(buf instanceof Buffer).toBe(true);
  expect(buf.length).toBe(0);
});

test("insertFromObjects inserts from objects", () => {
  const db = new Database("SchemaObjects4");
  db.createTable("users", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
    { name: "score", dataType: DataType.Integer },
  ]);

  const qb = db.createQueryBuilder();
  const n = qb.insertFromObjects("users", [
    { id: 0, name: "Alice", score: 95 },
    { id: 1, name: "Bob", score: 87 },
    { id: 2, name: "Charlie", score: 92 },
  ]);
  expect(n).toBe(3);

  const rows = qb.select("users").execute() as Array<any>;
  expect(rows.length).toBe(3);
  expect(rows[0].name).toBe("Alice");
  expect(rows[2].score).toBe(92);
});

test("insertFromObjects handles missing columns as null", () => {
  const db = new Database("SchemaObjects5");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertFromObjects("t", [{ id: 0 }]);
  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[0].val).toBe(null);
});

test("insertFromObjects empty array returns 0", () => {
  const db = new Database("SchemaObjects6");
  db.createTable("t", [{ name: "id", dataType: DataType.Integer }]);
  const qb = db.createQueryBuilder();
  expect(qb.insertFromObjects("t", [])).toBe(0);
});

test("updateFromObjects updates rows by id", () => {
  const db = new Database("SchemaObjects7");
  db.createTable("users", [
    { name: "id", dataType: DataType.Integer },
    { name: "score", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertFromObjects("users", [
    { id: 0, score: 100, name: "Alice" },
    { id: 1, score: 200, name: "Bob" },
    { id: 2, score: 300, name: "Charlie" },
  ]);

  const n = qb.updateFromObjects("users", [
    { id: 1, score: 999 },
    { id: 2, name: "Chuck" },
  ]);
  expect(n).toBe(2);

  const rows = qb.select("users").execute() as Array<any>;
  expect(rows[0].score).toBe(100); // unchanged
  expect(rows[1].score).toBe(999); // updated
  expect(rows[1].name).toBe("Bob"); // unchanged
  expect(rows[2].name).toBe("Chuck"); // updated
  expect(rows[2].score).toBe(300); // unchanged
});

test("updateFromObjects handles missing ids gracefully", () => {
  const db = new Database("SchemaObjects8");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, 10, 1, 20], 2);

  // One valid, one non-existent
  const n = qb.updateFromObjects("t", [
    { id: 0, val: 99 },
    { id: 999, val: 999 },
  ]);
  expect(n).toBe(1);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[0].val).toBe(99);
  expect(rows[1].val).toBe(20); // unchanged
});

test("objectsToArrowIpc buffer works with apache-arrow roundtrip", () => {
  const db = new Database("SchemaObjects9");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertFromObjects("t", [{ id: 0, name: "Alice" }, { id: 1, name: "Bob" }]);

  // Use executeArrow to get data back, then objectsToArrowIpc to create update buffer
  const buf = qb.select("t").executeArrow() as Buffer;
  expect(buf.length).toBeGreaterThan(0);

  // Create update objects for both rows
  const updateBuf = db.objectsToArrowIpc("t", [
    { id: 0, name: "Alicia" },
    { id: 1, name: "Robert" },
  ]);
  expect(updateBuf.length).toBeGreaterThan(0);

  const n = qb.updateFromArrow("t", updateBuf);
  expect(n).toBe(2);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[0].name).toBe("Alicia");
  expect(rows[1].name).toBe("Robert");
});

test("updateFromObjects with mixed types", () => {
  const db = new Database("SchemaObjects10");
  db.createTable("items", [
    { name: "id", dataType: DataType.Integer },
    { name: "price", dataType: DataType.Float },
    { name: "active", dataType: DataType.Boolean },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertFromObjects("items", [
    { id: 0, price: 10.0, active: true },
    { id: 1, price: 20.0, active: false },
  ]);

  qb.updateFromObjects("items", [
    { id: 0, price: 15.5, active: false },
  ]);

  const rows = qb.select("items").execute() as Array<any>;
  expect(rows[0].price).toBeCloseTo(15.5);
  expect(rows[0].active).toBe(false);
  expect(rows[1].price).toBeCloseTo(20.0);
  expect(rows[1].active).toBe(false);
});

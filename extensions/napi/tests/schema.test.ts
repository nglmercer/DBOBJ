import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("schema.getColumnNames", () => {
  const db = new Database("Test_SchemaCols");
  db.createTable("t", [
    { name: "a", dataType: DataType.Integer },
    { name: "b", dataType: DataType.String },
  ]);
  const names = db.schema.getColumnNames("t");
  expect(names.length).toBe(2);
  expect(names).toContain("a");
  expect(names).toContain("b");
});

test("schema.getColumnType", () => {
  const db = new Database("Test_SchemaType");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
    { name: "active", dataType: DataType.Boolean },
    { name: "score", dataType: DataType.Float },
  ]);
  expect(db.schema.getColumnType("t", "id")).toBe(DataType.Integer);
  expect(db.schema.getColumnType("t", "name")).toBe(DataType.String);
  expect(db.schema.getColumnType("t", "active")).toBe(DataType.Boolean);
  expect(db.schema.getColumnType("t", "score")).toBe(DataType.Float);
});

test("schema.hasColumn", () => {
  const db = new Database("Test_SchemaHas");
  db.createTable("t", [{ name: "x", dataType: DataType.Integer }]);
  expect(db.schema.hasColumn("t", "x")).toBe(true);
  expect(db.schema.hasColumn("t", "y")).toBe(false);
});

test("schema throws on missing table", () => {
  const db = new Database("Test_SchemaMiss");
  expect(() => db.schema.getColumnNames("nope")).toThrow();
  expect(() => db.schema.getColumnType("nope", "x")).toThrow();
  expect(() => db.schema.hasColumn("nope", "x")).toThrow();
});

test("schema throws on missing column", () => {
  const db = new Database("Test_SchemaBadCol");
  db.createTable("t", [{ name: "x", dataType: DataType.Integer }]);
  expect(() => db.schema.getColumnType("t", "y")).toThrow();
});

test("schema.validateRow valid", () => {
  const db = new Database("Test_Validate");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer, nullable: false },
    { name: "name", dataType: DataType.String, nullable: false },
    { name: "age", dataType: DataType.Integer, nullable: true },
  ]);
  const errors = db.schema.validateRow("t", [1, "bob", 30]);
  expect(errors.length).toBe(0);
});

test("schema.validateRow type mismatch", () => {
  const db = new Database("Test_ValidateErr");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer, nullable: false },
    { name: "name", dataType: DataType.String, nullable: false },
  ]);
  const errors = db.schema.validateRow("t", ["not-a-number", 42]);
  expect(errors.length).toBeGreaterThan(0);
  expect(errors[0]).toContain("id");
  expect(errors[0]).toContain("Integer");
});

test("schema.validateRow nullable accepts null", () => {
  const db = new Database("Test_ValidateNullable");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer, nullable: false },
    { name: "name", dataType: DataType.String, nullable: true },
  ]);
  const errors = db.schema.validateRow("t", [1, null]);
  expect(errors.length).toBe(0);
});

test("schema.validateRow wrong count", () => {
  const db = new Database("Test_ValidateCount");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);
  const errors = db.schema.validateRow("t", [1]);
  expect(errors.length).toBeGreaterThan(0);
  expect(errors[0]).toContain("expected 2 values");
});

test("schema.validateRow throws on missing table", () => {
  const db = new Database("Test_ValidateMiss");
  expect(() => db.schema.validateRow("nope", [1])).toThrow();
});

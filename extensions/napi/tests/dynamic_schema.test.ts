import { expect, test } from "bun:test";
const { DynamicSchema, DataType } = require("../index.js") as typeof import("../index.d.ts");

const fields = [
  { name: "id", type: DataType.Integer },
  { name: "name", type: DataType.String },
  { name: "optional_field", type: DataType.String, optional: true },
];

test("register and validate", () => {
  const ds = new DynamicSchema();
  ds.register("test_schema", fields);

  const valid = ds.validate("test_schema", [
    { id: 1, name: "alice", optional_field: "hello" },
    { id: 2, name: "bob" },
  ]);
  expect(valid).toEqual([
    { id: 1, name: "alice", optional_field: "hello" },
    { id: 2, name: "bob", optional_field: null },
  ]);

  expect(() =>
    ds.validate("test_schema", [{ id: "wrong" }]),
  ).toThrow();

  expect(() =>
    ds.validate("test_schema", [{ name: "no_id" }]),
  ).toThrow();
});

test("validateObject", () => {
  const ds = new DynamicSchema();
  ds.register("test", [
    { name: "id", type: DataType.Integer },
    { name: "name", type: DataType.String },
  ]);
  expect(() => ds.validateObject("test", { id: 1, name: "x" })).not.toThrow();
  expect(() => ds.validateObject("test", { id: 1 })).toThrow();
  expect(() => ds.validateObject("test", { id: "foo", name: "x" })).toThrow();
});

test("parse and parseString", () => {
  const ds = new DynamicSchema();
  ds.register("test", [
    { name: "id", type: DataType.Integer },
    { name: "name", type: DataType.String },
  ]);
  const input = JSON.stringify([{ id: 1, name: "x" }, { id: 2, name: "y" }]);
  const parsed = ds.parseString("test", input);
  expect(parsed).toEqual([{ id: 1, name: "x" }, { id: 2, name: "y" }]);

  const buf = Buffer.from(input);
  expect(ds.parse("test", buf)).toEqual(parsed);
});

test("toRowValues", () => {
  const ds = new DynamicSchema();
  ds.register("test", [
    { name: "id", type: DataType.Integer },
    { name: "name", type: DataType.String },
    { name: "opt", type: DataType.Integer, optional: true },
  ]);
  const row = ds.toRowValues("test", { id: 42, name: "hello" });
  expect(row).toEqual([42, "hello", null]);
});

test("validateObject with arrays", () => {
  const ds = new DynamicSchema();
  ds.register("test", [
    { name: "tags", type: DataType.ArrayString },
  ]);
  expect(() => ds.validateObject("test", { tags: ["a", "b"] })).not.toThrow();
  expect(() => ds.validateObject("test", { tags: "not-array" })).toThrow();
  expect(() => ds.validateObject("test", { tags: [1, 2] })).toThrow();
});

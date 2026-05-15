import { expect, test, describe } from "bun:test";
import { DynamicSchema, FieldType } from "../index";

describe("DynamicSchema", () => {
  test("register and validate", () => {
    const ds = new DynamicSchema();
    ds.register("user", [
      { name: "id", type: FieldType.I64 },
      { name: "name", type: FieldType.String },
      { name: "optional_field", type: FieldType.String, optional: true },
    ]);

    const valid = { id: 1, name: "Alice" };
    expect(ds.validate("user", valid)).toEqual({ id: 1, name: "Alice", optional_field: null });

    const invalid = { id: "1", name: "Alice" };
    expect(() => ds.validate("user", invalid)).toThrow();

    const missing = { id: 1 };
    expect(() => ds.validate("user", missing)).toThrow();
  });

  test("validateObject", () => {
    const ds = new DynamicSchema();
    ds.register("user", [
      { name: "id", type: FieldType.I64 },
      { name: "name", type: FieldType.String },
    ]);

    const obj = { id: 1, name: "Alice" };
    expect(ds.validateObject("user", obj)).toBe(obj);

    expect(() => ds.validateObject("user", { id: "1", name: "Alice" })).toThrow();
  });

  test("parse and parseString", () => {
    const ds = new DynamicSchema();
    ds.register("user", [
      { name: "id", type: FieldType.I64 },
      { name: "name", type: FieldType.String },
    ]);

    const json = JSON.stringify([{ id: 1, name: "Alice" }, { id: 2, name: "Bob" }]);
    const result = ds.parseString("user", json);
    expect(result).toEqual([{ id: 1, name: "Alice" }, { id: 2, name: "Bob" }]);

    const buffer = Buffer.from(json);
    expect(ds.parse("user", buffer)).toEqual(result);
  });

  test("toRowValues", () => {
      const ds = new DynamicSchema();
      ds.register("user", [
        { name: "id", type: FieldType.I64 },
        { name: "name", type: FieldType.String },
      ]);
      const obj = { id: 1, name: "Alice" };
      expect(ds.toRowValues("user", obj)).toEqual([1, "Alice"]);
  });

  test("validateObject with arrays", () => {
    const ds = new DynamicSchema();
    ds.register("test", [
      { name: "tags", type: FieldType.ArrayString },
    ]);

    expect(ds.validateObject("test", { tags: ["a", "b"] })).toBeDefined();
    expect(() => ds.validateObject("test", { tags: [1, 2] })).toThrow();
  });
});

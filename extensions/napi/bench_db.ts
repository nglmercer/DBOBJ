import { Database, DynamicSchema, DataType } from "./index";

// Generate 100k rows
const COUNT = 100_000;
const schema: Array<{ name: string; type: DataType; optional?: boolean }> = [
  { name: "id", type: DataType.Integer },
  { name: "name", type: DataType.String },
  { name: "active", type: DataType.Boolean },
  { name: "score", type: DataType.Float },
];

const ds = new DynamicSchema();
ds.register("row", schema);

const db = new Database(":memory:");
db.createTableFromSchema("direct", ds, "row");

const objects: Array<Record<string, number | string | boolean>> = [];
for (let i = 0; i < COUNT; i++) {
  objects.push({ id: i, name: `user_${i}`, active: i % 2 === 0, score: i * 1.5 });
}
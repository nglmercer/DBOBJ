import { DynamicSchema, DataType } from "./index";

const fields = [
  { name: "id", type: DataType.Integer },
  { name: "name", type: DataType.String },
  { name: "active", type: DataType.Boolean },
  { name: "score", type: DataType.Float },
];

const ds = new DynamicSchema();
ds.register("test", fields);

const data = Array.from({ length: 5000 }, (_, i) => ({
  id: i,
  name: `user_${i}`,
  active: i % 2 === 0,
  score: i * 1.5,
}));
const jsonStr = JSON.stringify(data);
const buf = Buffer.from(jsonStr);


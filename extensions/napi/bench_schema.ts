import { DynamicSchema, FieldType } from "./index";

const schemaName = "bench_schema";
const fields = [
  { name: "id", type: FieldType.I64 },
  { name: "name", type: FieldType.String },
  { name: "active", type: FieldType.Bool },
  { name: "score", type: FieldType.F64 },
];

const ds = new DynamicSchema();
ds.register(schemaName, fields);

const rowCount = 10000;
const data = Array.from({ length: rowCount }, (_, i) => ({
  id: i,
  name: `User ${i}`,
  active: i % 2 === 0,
  score: i * 1.5,
}));

const jsonString = JSON.stringify(data);
const jsonBuffer = Buffer.from(jsonString);

function bench(name: string, fn: () => void, iterations: number = 100) {
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const end = performance.now();
  console.log(`${name}: ${(end - start).toFixed(2)}ms (total for ${iterations} iterations)`);
}

console.log(`Benchmarking ${rowCount} rows...`);

bench("JSON.parse", () => {
  JSON.parse(jsonString);
});

bench("DynamicSchema.parseString", () => {
  ds.parseString(schemaName, jsonString);
});

bench("DynamicSchema.parse", () => {
  ds.parse(schemaName, jsonBuffer);
});

const parsed = JSON.parse(jsonString);
bench("DynamicSchema.validate", () => {
  ds.validate(schemaName, parsed);
});

bench("DynamicSchema.validateObject", () => {
  for (const obj of parsed) {
    ds.validateObject(schemaName, obj);
  }
});

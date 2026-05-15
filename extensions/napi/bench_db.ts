import { Database, DynamicSchema, FieldType } from "./index";

const dbName = ":memory:";
const schemaName = "bench_schema";
const tableName = "users";

const ds = new DynamicSchema();
const fields = [
  { name: "id", type: FieldType.I64 },
  { name: "name", type: FieldType.String },
  { name: "active", type: FieldType.Bool },
  { name: "score", type: FieldType.F64 },
];
ds.register(schemaName, fields);

const rowCount = 10000;
const data = Array.from({ length: rowCount }, (_, i) => ({
  id: i,
  name: `User ${i}`,
  active: i % 2 === 0,
  score: i * 1.5,
}));

function bench(name: string, fn: () => void, iterations: number = 50) {
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    fn();
  }
  const end = performance.now();
  console.log(`${name}: ${(end - start).toFixed(2)}ms (total for ${iterations} iterations)`);
}

const db = new Database(dbName);
db.createTableFromSchema(tableName, ds, schemaName);

console.log(`Benchmarking DB Insert with ${rowCount} rows...`);

bench("insertBatch (serde_json path)", () => {
  // We need to convert objects to flat array for original insertBatch
  const flatData = data.flatMap(obj => [obj.id, obj.name, obj.active, obj.score]);
  db.insertBatch(tableName, flatData, 4);
});

// Clear table
db.executeSql(`DELETE FROM ${tableName}`);

bench("insertBatchObjects (direct path)", () => {
  db.insertBatchObjects(tableName, data, ds, schemaName);
});

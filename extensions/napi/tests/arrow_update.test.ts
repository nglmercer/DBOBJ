import { expect, test } from "bun:test";
import { tableFromArrays, tableToIPC, tableFromIPC } from "apache-arrow";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("updateFromArrow updates rows by id", () => {
  const db = new Database("UpdateArrow1");
  db.createTable("users", [
    { name: "id", dataType: DataType.Integer },
    { name: "score", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertBatch("users", [0, 100, "Alice", 1, 200, "Bob", 2, 300, "Charlie"], 3);

  // Build Arrow buffer with Int64 for id/score columns
  const table = tableFromArrays({
    id: new BigInt64Array([1n, 2n]),
    score: new BigInt64Array([999n, 888n]),
  });
  const buf = tableToIPC(table, "file");

  const n = qb.updateFromArrow("users", buf);
  expect(n).toBe(2);

  const rows = qb.select("users").execute() as Array<any>;
  expect(rows[0].name).toBe("Alice");
  expect(rows[0].score).toBe(100);
  expect(rows[1].name).toBe("Bob");
  expect(rows[1].score).toBe(999);
  expect(rows[2].name).toBe("Charlie");
  expect(rows[2].score).toBe(888);
});

test("updateFromArrow with mixed types", () => {
  const db = new Database("UpdateArrow2");
  db.createTable("users", [
    { name: "id", dataType: DataType.Integer },
    { name: "score", dataType: DataType.Float },
    { name: "active", dataType: DataType.Boolean },
    { name: "name", dataType: DataType.String },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertBatch("users", [0, 90.0, true, "Alice", 1, 85.5, false, "Bob", 2, 95.0, true, "Charlie"], 4);

  // Update id=0: score=100, active=false; id=2: name="Chuck"
  const table = tableFromArrays({
    id: new BigInt64Array([0n, 2n]),
    score: new Float64Array([100.0, 95.0]),
    active: [false, true],
    name: ["Alice", "Chuck"],
  });
  const buf = tableToIPC(table, "file");

  const n = qb.updateFromArrow("users", buf);
  expect(n).toBe(2);

  const rows = qb.select("users").execute() as Array<any>;
  expect(rows[0].id).toBe(0);
  expect(rows[0].score).toBe(100.0);
  expect(rows[0].active).toBe(false);
  expect(rows[0].name).toBe("Alice");
  expect(rows[2].name).toBe("Chuck");
});

test("updateFromArrow on non-existent id returns 0", () => {
  const db = new Database("UpdateArrow3");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, 10, 1, 20], 2);

  const table = tableFromArrays({
    id: new BigInt64Array([99n]),
    val: new BigInt64Array([999n]),
  });
  const buf = tableToIPC(table, "file");

  const n = qb.updateFromArrow("t", buf);
  expect(n).toBe(0); // id=99 doesn't exist
});

test("updateFromArrow requires id column", () => {
  const db = new Database("UpdateArrow4");
  db.createTable("t", [{ name: "id", dataType: DataType.Integer }]);
  const qb = db.createQueryBuilder();

  const table = tableFromArrays({
    val: [1],
  });
  const buf = tableToIPC(table, "file");

  expect(() => qb.updateFromArrow("t", buf)).toThrow();
});

test("updateFromArrow empty buffer", () => {
  const db = new Database("UpdateArrow5");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, 10, 1, 20], 2);

  const table = tableFromArrays({
    id: new BigInt64Array([]),
    val: new BigInt64Array([]),
  });
  const buf = tableToIPC(table, "file");

  const n = qb.updateFromArrow("t", buf);
  expect(n).toBe(0);
});

test("executeArrow + updateFromArrow roundtrip", () => {
  const db = new Database("UpdateArrow6");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, 10, 1, 20, 2, 30], 2);

  // Use executeArrow to get current data, modify via apache-arrow, update back
  const buf = qb.select("t").executeArrow() as Buffer;

  // Parse with apache-arrow, modify val to 999 for all rows
  const table = tableFromIPC(new Uint8Array(buf));
  const idArr = table.getChild("id")!.toArray();
  const valArr = table.getChild("val")!.toArray();
  const newTable = tableFromArrays({
    id: [...idArr],
    val: valArr.map(() => 999n),
  });
  const updateBuf = tableToIPC(newTable, "file");

  const n = qb.updateFromArrow("t", updateBuf);
  expect(n).toBe(3);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[0].val).toBe(999);
  expect(rows[1].val).toBe(999);
  expect(rows[2].val).toBe(999);
});

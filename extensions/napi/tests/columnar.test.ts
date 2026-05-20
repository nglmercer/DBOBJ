import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("insertColumnar with BigInt64Array", () => {
  const db = new Database("ColInsert1");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);
  const qb = db.createQueryBuilder();

  const n = qb.insertColumnar("t", {
    id: new BigInt64Array([10n, 20n, 30n]),
    val: new BigInt64Array([100n, 200n, 300n]),
  });
  expect(n).toBe(3);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows.length).toBe(3);
  expect(rows[0].id).toBe(10);
  expect(rows[0].val).toBe(100);
  expect(rows[2].val).toBe(300);
});

test("insertColumnar with Float64Array", () => {
  const db = new Database("ColInsert2");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "score", dataType: DataType.Float },
  ]);
  const qb = db.createQueryBuilder();

  const n = qb.insertColumnar("t", {
    id: new BigInt64Array([1n, 2n]),
    score: new Float64Array([95.5, 87.3]),
  });
  expect(n).toBe(2);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[0].score).toBe(95.5);
});

test("insertColumnar with mixed types", () => {
  const db = new Database("ColInsert3");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
    { name: "active", dataType: DataType.Boolean },
  ]);
  const qb = db.createQueryBuilder();

  const n = qb.insertColumnar("t", {
    id: new BigInt64Array([1n, 2n]),
    name: ["Alice", "Bob"],
    active: [true, false],
  });
  expect(n).toBe(2);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[0].name).toBe("Alice");
  expect(rows[0].active).toBe(true);
  expect(rows[1].active).toBe(false);
});

test("insertColumnar empty input returns 0", () => {
  const db = new Database("ColInsert4");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
  ]);
  const qb = db.createQueryBuilder();

  const n = qb.insertColumnar("t", {
    id: new BigInt64Array([]),
  });
  expect(n).toBe(0);
});

test("updateColumnar with BigInt64Array", () => {
  const db = new Database("ColUpdate1");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);
  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, 10, 1, 20, 2, 30], 2);

  const n = qb.updateColumnar("t", {
    id: new BigInt64Array([1n, 2n]),
    val: new BigInt64Array([999n, 888n]),
  });
  expect(n).toBe(2);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[0].val).toBe(10);
  expect(rows[1].val).toBe(999);
  expect(rows[2].val).toBe(888);
});

test("updateColumnar partial columns (merge)", () => {
  const db = new Database("ColUpdate2");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);
  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, 10, 1, 20, 2, 30], 2);

  // Only update val for id=1, no other columns provided
  const n = qb.updateColumnar("t", {
    id: new BigInt64Array([1n]),
    val: new BigInt64Array([999n]),
  });
  expect(n).toBe(1);
  expect((qb.select("t").execute() as Array<any>)[1].val).toBe(999);
});

test("updateColumnar with string/boolean columns", () => {
  const db = new Database("ColUpdate3");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "name", dataType: DataType.String },
    { name: "active", dataType: DataType.Boolean },
  ]);
  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, "Alice", true, 1, "Bob", false], 3);

  const n = qb.updateColumnar("t", {
    id: new BigInt64Array([1n]),
    name: ["Robert"],
    active: [true],
  });
  expect(n).toBe(1);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[1].name).toBe("Robert");
  expect(rows[1].active).toBe(true);
});

test("updateColumnar non-existent id returns 0", () => {
  const db = new Database("ColUpdate4");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);
  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, 10], 2);

  const n = qb.updateColumnar("t", {
    id: new BigInt64Array([99n]),
    val: new BigInt64Array([999n]),
  });
  expect(n).toBe(0);
});

test("updateColumnar requires id column", () => {
  const db = new Database("ColUpdate5");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);
  const qb = db.createQueryBuilder();

  expect(() => qb.updateColumnar("t", {
    val: new BigInt64Array([999n]),
  })).toThrow();
});

test("updateFromArrow with Uint8Array (not Buffer)", () => {
  const db = new Database("ArrowUint8");
  db.createTable("t", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);
  const qb = db.createQueryBuilder();
  qb.insertBatch("t", [0, 10, 1, 20], 2);

  // Get data as Arrow, modify, write back via updateFromArrow
  const buf = qb.select("t").executeArrow() as Buffer;

  // Import apache-arrow, modify val to 999, export as Uint8Array
  const { tableFromIPC, tableFromArrays, tableToIPC } = require("apache-arrow");
  const table = tableFromIPC(new Uint8Array(buf));
  const newTable = tableFromArrays({
    id: [...table.getChild("id")!.toArray()],
    val: table.getChild("val")!.toArray().map(() => 999n),
  });
  const uint8Buf = tableToIPC(newTable, "file") as Uint8Array;

  // Pass Uint8Array (not Buffer) — should still work
  const n = qb.updateFromArrow("t", uint8Buf);
  expect(n).toBe(2);

  const rows = qb.select("t").execute() as Array<any>;
  expect(rows[0].val).toBe(999);
  expect(rows[1].val).toBe(999);
});

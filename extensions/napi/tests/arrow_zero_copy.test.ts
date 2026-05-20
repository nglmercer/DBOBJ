import { expect, test } from "bun:test";
const { Database, DataType } = require("../index.js") as typeof import("../index.d.ts");

test("create table from Arrow schema", () => {
  const db = new Database("ArrowSchemaTest");
  db.createTable("orig", [
    { name: "id", dataType: DataType.Integer },
    { name: "val", dataType: DataType.Integer },
  ]);
  const buf = db.exportTableToArrowIpc("orig");
  const cols = db.createTableFromArrowIpc("reborn", buf);
  expect(cols).toBe(2);

  const flat = new BigInt64Array(6);
  flat[0] = 0n; flat[1] = 100n;
  flat[2] = 1n; flat[3] = 200n;
  flat[4] = 2n; flat[5] = 300n;
  const qb = db.createQueryBuilder();
  qb.insertBatchI64("orig", flat, 2);

  const arrowBuf = qb.select("orig").executeArrow() as Buffer;
  expect(arrowBuf.byteLength).toBeGreaterThan(0);

  const inserted = qb.insertFromArrow("reborn", arrowBuf);
  expect(inserted).toBe(3);

  const rows = qb.select("reborn").execute() as Array<any>;
  expect(rows.length).toBe(3);
  expect(rows[0].val).toBe(100);
});

test("Arrow zero-copy roundtrip with mixed types", () => {
  const db = new Database("ArrowRoundtrip");
  db.createTable("users", [
    { name: "id", dataType: DataType.Integer },
    { name: "score", dataType: DataType.Float },
    { name: "active", dataType: DataType.Boolean },
    { name: "name", dataType: DataType.String },
  ]);

  const qb = db.createQueryBuilder();
  qb.insertBatch("users", [1, 95.5, true, "Alice", 2, 87.3, false, "Bob"], 4);

  const buf = qb.select("users").executeArrow() as Buffer;
  const cols = db.createTableFromArrowIpc("users2", buf);
  expect(cols).toBe(4);

  const n = qb.insertFromArrow("users2", buf);
  expect(n).toBe(2);

  const rows = qb.select("users2").execute() as Array<any>;
  expect(rows.length).toBe(2);
  expect(rows[0].name).toBe("Alice");
  expect(rows[0].score).toBe(95.5);
});

test("executeArrow on empty result returns empty buffer", () => {
  const db = new Database("ArrowEmpty");
  db.createTable("t", [{ name: "x", dataType: DataType.Integer }]);
  const qb = db.createQueryBuilder();
  const buf = qb.select("t").whereEq("x", 999).executeArrow() as Buffer;
  expect(buf.byteLength).toBe(0);
});

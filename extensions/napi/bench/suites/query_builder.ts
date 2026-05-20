const { Database, DataType } = require("../../index.js") as typeof import("../../index.d.ts");
import { tableFromArrays, tableToIPC } from "apache-arrow";
import { TestSuite } from "../interface";
import { JOIN_COUNT } from "../constants";

export class DBOBJQueryBuilderSuite implements TestSuite {
  name = "DBOBJ QueryBuilder";
  db = new Database("qb");

  insert(count: number) {
    this.db.createTable("users", [
      { name: "id", dataType: DataType.Integer },
      { name: "val", dataType: DataType.Integer },
    ]);
    this.db.createTable("names", [
      { name: "name", dataType: DataType.String },
    ]);
    this.db.createTable("actives", [
      { name: "active", dataType: DataType.Boolean },
    ]);

    const batchSize = 20000;
    const batches = Math.ceil(count / batchSize);
    const t0 = performance.now();
    const qb = this.db.createQueryBuilder();

    for (let b = 0; b < batches; b++) {
      const start = b * batchSize;
      const end = Math.min(start + batchSize, count);
      const batchLen = end - start;

      const ids = new BigInt64Array(batchLen);
      const vals = new BigInt64Array(batchLen);
      const names: string[] = new Array(batchLen);
      const actives: boolean[] = new Array(batchLen);
      for (let i = 0; i < batchLen; i++) {
        const idx = start + i;
        ids[i] = BigInt(idx);
        vals[i] = BigInt(idx * 10);
        names[i] = `user_${idx}`;
        actives[i] = idx % 2 === 0;
      }

      qb.insertColumnar("users", { id: ids, val: vals });
      qb.insertColumnar("names", { name: names });
      qb.insertColumnar("actives", { active: actives });
    }
    const elapsed = performance.now() - t0;

    const rows = qb.select("users").execute() as Array<any>;
    if (rows.length !== count) throw new Error(`users count ${rows.length} !== ${count}`);

    return elapsed;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    const qb = this.db.createQueryBuilder();
    const rows = qb.select(tableName).executeColumnar() as Record<string, any[]>;
    const elapsed = performance.now() - t0;
    const col = rows[colName];
    if (!col || col.length === 0) throw new Error(`column ${colName} is empty`);
    return elapsed;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    const qb = this.db.createQueryBuilder();
    const res = qb.select(tableName).whereEq(colName, value).execute() as Array<Record<string, any>>;
    const elapsed = performance.now() - t0;
    if (res.length === 0) throw new Error(`find ${colName}=${value} returned empty`);
    if (Number(res[0].id) !== value) throw new Error(`find id mismatch: ${res[0].id} !== ${value}`);
    return elapsed;
  }

  update(tableName: string, count: number) {
    const batchSize = 5000;
    const batches = Math.ceil(count / batchSize);
    const t0 = performance.now();
    const qb = this.db.createQueryBuilder();

    const idArr = new BigInt64Array(count);
    const valArr = new BigInt64Array(count);
    for (let i = 0; i < count; i++) {
      idArr[i] = BigInt(i);
      valArr[i] = BigInt(i * 20);
    }

    for (let b = 0; b < batches; b++) {
      const start = b * batchSize;
      const end = Math.min(start + batchSize, count);
      const batchIds = idArr.subarray(start, end);
      const batchVals = valArr.subarray(start, end);
      qb.updateColumnar(tableName, { id: batchIds, val: batchVals });
    }

    const elapsed = performance.now() - t0;

    const row = qb.select(tableName).whereEq("id", 50).first() as any;
    if (!row) throw new Error("update row not found");
    if (Number(row.val) !== 1000) throw new Error(`update val mismatch: ${row.val} !== 1000`);
    return elapsed;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.createTable(t2, [
      { name: "id", dataType: DataType.Integer },
      { name: "score", dataType: DataType.Integer },
    ]);
    const qb = this.db.createQueryBuilder();
    const batchSize = 5000;
    const batches = Math.ceil(JOIN_COUNT / batchSize);
    for (let b = 0; b < batches; b++) {
      const start = b * batchSize;
      const end = Math.min(start + batchSize, JOIN_COUNT);
      const vals: number[] = [];
      for (let i = start; i < end; i++) vals.push(i, i + 5);
      qb.insertBatch(t2, vals, 2);
    }

    const t0 = performance.now();
    const res = qb.select(t1).join(t2, c1, c2).execute() as Array<Record<string, any>>;
    const elapsed = performance.now() - t0;

    if (res.length !== JOIN_COUNT) throw new Error(`join result count ${res.length} !== ${JOIN_COUNT}`);
    if (Number(res[0].val) !== 0) throw new Error(`join val mismatch: ${res[0].val} !== 0`);
    return elapsed;
  }
}

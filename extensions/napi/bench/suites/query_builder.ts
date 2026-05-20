const { Database } = require("../../index.node") as typeof import("../../index.d.ts");
import { TestSuite } from "../interface";
import { JOIN_COUNT } from "../constants";

export class DBOBJQueryBuilderSuite implements TestSuite {
  name = "DBOBJ QueryBuilder";
  db = new Database("qb");

  insert(count: number) {
    this.db.executeSql("CREATE TABLE users (id INTEGER, val INTEGER)");
    this.db.executeSql("CREATE TABLE names (name STRING)");
    this.db.executeSql("CREATE TABLE actives (active BOOLEAN)");

    const batchSize = 20000;
    const batches = Math.ceil(count / batchSize);
    const t0 = performance.now();
    const qb = this.db.createQueryBuilder();

    for (let b = 0; b < batches; b++) {
      const start = b * batchSize;
      const end = Math.min(start + batchSize, count);
      const chunkSize = end - start;

      // Build flat row-major arrays for each table
      const intVals: number[] = [];
      const strVals: string[] = [];
      const boolVals: boolean[] = [];
      for (let i = start; i < end; i++) {
        intVals.push(i, i * 10);
        strVals.push(`user_${i}`);
        boolVals.push(i % 2 === 0);
      }

      qb.insertBatch("users", intVals, 2);
      qb.insertBatch("names", strVals, 1);
      qb.insertBatch("actives", boolVals, 1);
    }
    const elapsed = performance.now() - t0;

    const users = this.db.executeSql("SELECT COUNT(*) FROM users") as Array<Record<string, any>>;
    const userCount = Number(users[0]["COUNT(*)"]);
    if (userCount !== count) throw new Error(`users count ${userCount} !== ${count}`);

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

    for (let b = 0; b < batches; b++) {
      const start = b * batchSize;
      const end = Math.min(start + batchSize, count);
      for (let i = start; i < end; i++) {
        qb.update(tableName).set("val", i * 20).whereEq("id", i).execute();
      }
    }
    const elapsed = performance.now() - t0;

    const res = this.db.executeSql("SELECT val FROM users WHERE id = 50") as Array<Record<string, any>>;
    if (Number(res[0].val) !== 1000) throw new Error(`update val mismatch: ${res[0].val} !== 1000`);
    return elapsed;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.executeSql(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
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

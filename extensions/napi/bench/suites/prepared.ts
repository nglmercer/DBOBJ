const { Database, DataType } = require("../../index.node") as typeof import("../../index.d.ts");
import { TestSuite } from "../interface";
import { JOIN_COUNT } from "../constants";

export class DBOBJSQLPreparedSuite implements TestSuite {
  name = "DBOBJ SQL (Prepared)";
  db = new Database("sql_prepared");

  insert(count: number) {
    this.db.executeSql("CREATE TABLE users (id INTEGER, val INTEGER)");
    this.db.executeSql("CREATE TABLE names (name STRING)");
    this.db.executeSql("CREATE TABLE actives (active BOOLEAN)");

    const intBatch = new BigInt64Array(count * 2);
    const strBatch = new Array<string>(count);
    const boolBatch = new Array<boolean>(count);
    for (let i = 0; i < count; i++) {
      intBatch[i * 2] = BigInt(i);
      intBatch[i * 2 + 1] = BigInt(i * 10);
      strBatch[i] = `user_${i}`;
      boolBatch[i] = i % 2 === 0;
    }

    const intStmt = this.db.prepare("INSERT INTO users (id, val) VALUES (?, ?)");
    const strStmt = this.db.prepare("INSERT INTO names (name) VALUES (?)");
    const boolStmt = this.db.prepare("INSERT INTO actives (active) VALUES (?)");

    const t0 = performance.now();
    intStmt.runBatchI64(intBatch, 2);
    strStmt.runBatchString(strBatch, 1);
    boolStmt.runBatchBool(boolBatch, 1);
    const elapsed = performance.now() - t0;

    const usersCount = this.db.countRows("users");
    const namesCount = this.db.countRows("names");
    const activesCount = this.db.countRows("actives");
    if (usersCount !== count) throw new Error(`users count ${usersCount} !== ${count}`);
    if (namesCount !== count) throw new Error(`names count ${namesCount} !== ${count}`);
    if (activesCount !== count) throw new Error(`actives count ${activesCount} !== ${count}`);

    return elapsed;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    const col = this.db.prepare(`SELECT ${colName} FROM ${tableName}`).allI64([]);
    const elapsed = performance.now() - t0;
    if (col.length === 0) throw new Error(`column ${colName} is empty`);
    return elapsed;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    const res = this.db.executeSql(`SELECT * FROM ${tableName} WHERE ${colName} = ${value}`) as Array<Record<string, any>>;
    const elapsed = performance.now() - t0;
    if (res.length === 0) throw new Error(`find ${colName}=${value} returned empty`);
    if (Number(res[0].id) !== value) throw new Error(`find id mismatch: ${res[0].id} !== ${value}`);
    return elapsed;
  }

  update(tableName: string, count: number) {
    const stmt = this.db.prepare(`UPDATE ${tableName} SET val = ? WHERE id = ?`);
    const t0 = performance.now();
    const batch = new BigInt64Array(count * 2);
    for (let i = 0; i < count; i++) {
      batch[i * 2] = BigInt(i * 20);
      batch[i * 2 + 1] = BigInt(i);
    }
    stmt.runBatchI64(batch, 2);
    const elapsed = performance.now() - t0;

    const res = this.db.executeSql("SELECT val FROM users WHERE id = 50") as Array<Record<string, any>>;
    if (Number(res[0].val) !== 1000) throw new Error(`update val mismatch: ${res[0].val} !== 1000`);
    return elapsed;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.executeSql(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
    const stmt = this.db.prepare(`INSERT INTO ${t2} (id, score) VALUES (?, ?)`);
    const batch = new BigInt64Array(JOIN_COUNT * 2);
    for (let i = 0; i < JOIN_COUNT; i++) {
      batch[i * 2] = BigInt(i);
      batch[i * 2 + 1] = BigInt(i + 5);
    }
    stmt.runBatchI64(batch, 2);

    const t0 = performance.now();
    const result = this.db.queryJoinI64(`SELECT * FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`);
    const elapsed = performance.now() - t0;

    if (result.length !== JOIN_COUNT * 4) throw new Error(`join result length ${result.length} !== ${JOIN_COUNT * 4}`);
    return elapsed;
  }
}

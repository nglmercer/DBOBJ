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
    strStmt.runBatchValues(strBatch, 1);
    boolStmt.runBatchValues(boolBatch, 1);
    return performance.now() - t0;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    this.db.prepare(`SELECT ${colName} FROM ${tableName}`).allI64([]);
    return performance.now() - t0;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    this.db.executeSql(`SELECT * FROM ${tableName} WHERE ${colName} = ${value}`);
    return performance.now() - t0;
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
    return performance.now() - t0;
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
    this.db.queryJoinI64(`SELECT * FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`);
    return performance.now() - t0;
  }
}

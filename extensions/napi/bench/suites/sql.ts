const { Database, DataType } = require("../../index.node") as typeof import("../../index.d.ts");
import { TestSuite } from "../interface";
import { JOIN_COUNT } from "../constants";

export class DBOBJSQLSuite implements TestSuite {
  name = "DBOBJ SQL (Engine)";
  db = new Database("sql");

  insert(count: number) {
    this.db.executeSql("CREATE TABLE users (id INTEGER, val INTEGER)");
    this.db.executeSql("CREATE TABLE names (name STRING)");
    this.db.executeSql("CREATE TABLE actives (active BOOLEAN)");

    const t0 = performance.now();
    const batchSize = 20000;
    const batches = Math.ceil(count / batchSize);
    for (let b = 0; b < batches; b++) {
      const start = b * batchSize;
      const end = Math.min(start + batchSize, count);

      let intSql = "";
      let strSql = "";
      let boolSql = "";
      for (let i = start; i < end; i++) {
        if (i > start) {
          intSql += ",";
          strSql += ",";
          boolSql += ",";
        }
        intSql += `(${i},${i * 10})`;
        strSql += `('user_${i}')`;
        boolSql += `(${i % 2 === 0 ? 1 : 0})`;
      }
      this.db.executeSql(`INSERT INTO users (id,val) VALUES ${intSql}`);
      this.db.executeSql(`INSERT INTO names (name) VALUES ${strSql}`);
      this.db.executeSql(`INSERT INTO actives (active) VALUES ${boolSql}`);
    }
    const elapsed = performance.now() - t0;

    const users = this.db.executeSql("SELECT COUNT(*) FROM users") as Array<Record<string, any>>;
    const userCount = Number(users[0]["COUNT(*)"]);
    if (userCount !== count) throw new Error(`users count ${userCount} !== ${count}`);

    return elapsed;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    const col = this.db.queryI64(`SELECT ${colName} FROM ${tableName}`);
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
    const t0 = performance.now();
    for (let i = 0; i < count; i++) {
      this.db.executeSql(`UPDATE ${tableName} SET val = ${i * 20} WHERE id = ${i}`);
    }
    const elapsed = performance.now() - t0;

    const res = this.db.executeSql("SELECT val FROM users WHERE id = 50") as Array<Record<string, any>>;
    if (Number(res[0].val) !== 1000) throw new Error(`update val mismatch: ${res[0].val} !== 1000`);
    return elapsed;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.executeSql(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
    for (let i = 0; i < JOIN_COUNT; i++) this.db.executeSql(`INSERT INTO ${t2} (id, score) VALUES (${i}, ${i + 5})`);

    const t0 = performance.now();
    const res = this.db.executeSql(`SELECT ${t1}.id, ${t1}.val, ${t2}.score FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`) as Array<Record<string, any>>;
    const elapsed = performance.now() - t0;

    if (res.length !== JOIN_COUNT) throw new Error(`join result count ${res.length} !== ${JOIN_COUNT}`);
    if (Number(res[0][`${t1}.val`]) !== 0) throw new Error(`join val mismatch: ${res[0][`${t1}.val`]} !== 0`);
    if (Number(res[0][`${t2}.score`]) !== 5) throw new Error(`join score mismatch: ${res[0][`${t2}.score`]} !== 5`);
    return elapsed;
  }
}

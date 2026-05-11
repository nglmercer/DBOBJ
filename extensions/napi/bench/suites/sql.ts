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
    const batchSize = 1000;
    const batches = Math.ceil(count / batchSize);
    for (let b = 0; b < batches; b++) {
      const start = b * batchSize;
      const end = Math.min(start + batchSize, count);

      const intVals: string[] = [];
      const strVals: string[] = [];
      const boolVals: string[] = [];
      for (let i = start; i < end; i++) {
        intVals.push(`(${i}, ${i * 10})`);
        strVals.push(`('user_${i}')`);
        boolVals.push(`(${i % 2 === 0 ? "TRUE" : "FALSE"})`);
      }
      this.db.executeSql(`INSERT INTO users (id, val) VALUES ${intVals.join(", ")}`);
      this.db.executeSql(`INSERT INTO names (name) VALUES ${strVals.join(", ")}`);
      this.db.executeSql(`INSERT INTO actives (active) VALUES ${boolVals.join(", ")}`);
    }
    return performance.now() - t0;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    this.db.queryI64(`SELECT ${colName} FROM ${tableName}`);
    return performance.now() - t0;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    this.db.executeSql(`SELECT * FROM ${tableName} WHERE ${colName} = ${value}`);
    return performance.now() - t0;
  }

  update(tableName: string, count: number) {
    const t0 = performance.now();
    for (let i = 0; i < count; i++) {
      this.db.executeSql(`UPDATE ${tableName} SET val = ${i * 20} WHERE id = ${i}`);
    }
    return performance.now() - t0;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.executeSql(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
    for (let i = 0; i < JOIN_COUNT; i++) this.db.executeSql(`INSERT INTO ${t2} (id, score) VALUES (${i}, ${i + 5})`);

    const t0 = performance.now();
    this.db.executeSql(`SELECT * FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`);
    return performance.now() - t0;
  }
}

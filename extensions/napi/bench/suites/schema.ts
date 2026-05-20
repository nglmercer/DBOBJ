const { Database, DataType, DynamicSchema } = require("../../index.js") as typeof import("../../index.d.ts");
import { TestSuite } from "../interface";
import { JOIN_COUNT } from "../constants";

export { Database, DataType, DynamicSchema };

export class DBOBJSchemaSuite implements TestSuite {
  name = "DBOBJ Schema (Direct)";
  db = new Database("schema");
  ds = new DynamicSchema();
  schemaName = "userSchema";

  constructor() {
    this.ds.register(this.schemaName, [
      { name: "id", type: DataType.Integer },
      { name: "val", type: DataType.Integer },
    ]);
    this.ds.register("nameSchema", [{ name: "name", type: DataType.String }]);
    this.ds.register("activeSchema", [{ name: "active", type: DataType.Boolean }]);
    this.ds.register("statsSchema", [
      { name: "id", type: DataType.Integer },
      { name: "score", type: DataType.Integer },
    ]);
  }

  insert(count: number) {
    this.db.createTableFromSchema("users", this.ds, this.schemaName);
    this.db.createTableFromSchema("names", this.ds, "nameSchema");
    this.db.createTableFromSchema("actives", this.ds, "activeSchema");

    const users = new Array(count);
    const names = new Array(count);
    const actives = new Array(count);

    for (let i = 0; i < count; i++) {
      users[i] = { id: i, val: i * 10 };
      names[i] = { name: `user_${i}` };
      actives[i] = { active: i % 2 === 0 };
    }

    const t0 = performance.now();
    this.db.insertBatchObjects("users", users, this.ds, this.schemaName);
    this.db.insertBatchObjects("names", names, this.ds, "nameSchema");
    this.db.insertBatchObjects("actives", actives, this.ds, "activeSchema");
    const elapsed = performance.now() - t0;

    const usersCount = this.db.countRows("users");
    if (usersCount !== count) throw new Error(`users count ${usersCount} !== ${count}`);

    return elapsed;
  }

  insertColumnar(count: number) {
    this.db.createTable("users_col", [
      { name: "id", dataType: DataType.Integer, nullable: false },
      { name: "val", dataType: DataType.Integer },
    ]);

    const ids = new BigInt64Array(count);
    const vals = new BigInt64Array(count);
    for (let i = 0; i < count; i++) {
      ids[i] = BigInt(i);
      vals[i] = BigInt(i * 10);
    }

    const t0 = performance.now();
    this.db.insertBatchColumnar("users_col", { id: ids, val: vals });
    const elapsed = performance.now() - t0;
    return elapsed;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    const col = this.db.getColumnI64(tableName, colName);
    const elapsed = performance.now() - t0;
    if (col.length === 0) throw new Error(`column ${colName} is empty`);
    return elapsed;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    const result = this.db.findByI64(tableName, colName, value);
    const elapsed = performance.now() - t0;
    if (result.length === 0) throw new Error(`find ${colName}=${value} returned empty`);
    return elapsed;
  }

  update(tableName: string, count: number) {
    const t0 = performance.now();
    for (let i = 0; i < count; i++) {
      this.db.updateObject(tableName, i, { id: i, val: i * 20 }, this.ds, this.schemaName);
    }
    const elapsed = performance.now() - t0;
    return elapsed;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.createTableFromSchema(t2, this.ds, "statsSchema");

    const stats = new Array(JOIN_COUNT);
    for (let i = 0; i < JOIN_COUNT; i++) {
      stats[i] = { id: i, score: i + 5 };
    }
    this.db.insertBatchObjects(t2, stats, this.ds, "statsSchema");

    const t0 = performance.now();
    const result = this.db.hashJoinI64(t1, c1, t2, c2);
    const elapsed = performance.now() - t0;
    return elapsed;
  }

  readColumnar(tableName: string, colName: string) {
    const t0 = performance.now();
    const col = this.db.getColumnI64(tableName, colName);
    const elapsed = performance.now() - t0;
    if (col.length === 0) throw new Error(`column ${colName} is empty`);
    return elapsed;
  }

  findColumnar(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    const result = this.db.findByI64(tableName, colName, value);
    const elapsed = performance.now() - t0;
    if (result.length === 0) throw new Error(`find ${colName}=${value} returned empty`);
    return elapsed;
  }

  updateColumnar(tableName: string, count: number) {
    const values = new BigInt64Array(count * 2);
    for (let i = 0; i < count; i++) {
      values[i * 2] = BigInt(i * 20); // new val
      values[i * 2 + 1] = BigInt(i);   // id
    }
    const t0 = performance.now();
    this.db.updateBatchI64(tableName, "val", values);
    const elapsed = performance.now() - t0;
    return elapsed;
  }

  joinColumnar(t1: string, c1: string, t2: string, c2: string) {
    const t0 = performance.now();
    const result = this.db.hashJoinI64(t1, c1, t2, c2);
    const elapsed = performance.now() - t0;
    return elapsed;
  }
}

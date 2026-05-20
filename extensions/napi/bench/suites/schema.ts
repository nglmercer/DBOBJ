const { Database: DBOBJ, DynamicSchema, DataType } = require("../../index.js") as typeof import("../../index.d.ts");
import { TestSuite } from "../interface";
import { JOIN_COUNT } from "../constants";

const SCHEMA_FIELDS = [
  { name: "id", type: DataType.Integer },
  { name: "val", type: DataType.Integer },
  { name: "name", type: DataType.String },
  { name: "active", type: DataType.Boolean },
];

export class DBOBJSchemaSuite implements TestSuite {
  name = "DBOBJ Schema (Direct)";
  db = new DBOBJ("direct_schema");

  insert(count: number) {
    const ds = new DynamicSchema();
    ds.register("row", SCHEMA_FIELDS);
    this.db.createTableFromSchema("users", ds, "row");

    const objects = new Array<Record<string, any>>(count);
    for (let i = 0; i < count; i++) {
      objects[i] = { id: i, val: i * 10, name: `user_${i}`, active: i % 2 === 0 };
    }
    const t0 = performance.now();
    this.db.insertBatchObjects("users", objects, ds, "row");
    const elapsed = performance.now() - t0;

    const userCount = this.db.countRows("users");
    if (userCount !== count) throw new Error(`users count ${userCount} !== ${count}`);

    return elapsed;
  }

  insertColumnar(count: number) {
    const ds = new DynamicSchema();
    ds.register("row", SCHEMA_FIELDS);
    this.db.createTableFromSchema("users_col", ds, "row");

    const ids = new BigInt64Array(count);
    const vals = new BigInt64Array(count);
    const names = new Array<string>(count);
    const actives = new Array<boolean>(count);
    for (let i = 0; i < count; i++) {
      ids[i] = BigInt(i);
      vals[i] = BigInt(i * 10);
      names[i] = `user_${i}`;
      actives[i] = i % 2 === 0;
    }
    const t0 = performance.now();
    this.db.insertBatchColumnar("users_col", { id: ids, val: vals, name: names, active: actives });
    const elapsed = performance.now() - t0;

    const userCount = this.db.countRows("users_col");
    if (userCount !== count) throw new Error(`users_col count ${userCount} !== ${count}`);

    return elapsed;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    const col = this.db.getColumnI64(tableName, colName);
    const elapsed = performance.now() - t0;
    if (col.length === 0) throw new Error(`column ${colName} is empty`);
    return elapsed;
  }

  readColumnar(tableName: string, colName: string) {
    return this.readColumn(tableName, colName);
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    const result = this.db.findByI64(tableName, colName, value);
    const elapsed = performance.now() - t0;
    if (result.length === 0) throw new Error(`find ${colName}=${value} returned empty`);
    if (Number(result[0]) !== value) throw new Error(`find result mismatch: ${result[0]} !== ${value}`);
    return elapsed;
  }

  findColumnar(tableName: string, colName: string, value: any) {
    return this.find(tableName, colName, value);
  }

  update(tableName: string, count: number) {
    const t0 = performance.now();
    for (let i = 0; i < count; i++) {
      this.db.updateRow(tableName, i, [i, i * 20, `user_${i}`, i % 2 === 0]);
    }
    const elapsed = performance.now() - t0;

    const col = this.db.getColumnI64(tableName, "val");
    for (let i = 0; i < count; i++) {
      if (Number(col[i]) !== i * 20) throw new Error(`update val mismatch at row ${i}: ${col[i]} !== ${i * 20}`);
    }
    return elapsed;
  }

  updateColumnar(tableName: string, count: number) {
    return this.update(tableName, count);
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.createTable(t2, [
      { name: "id", dataType: DataType.Integer, nullable: false },
      { name: "score", dataType: DataType.Integer },
    ]);
    const batch = new BigInt64Array(JOIN_COUNT * 2);
    for (let i = 0; i < JOIN_COUNT; i++) {
      batch[i * 2] = BigInt(i);
      batch[i * 2 + 1] = BigInt(i + 5);
    }
    this.db.insertBatchI64(t2, batch, 2);

    const t0 = performance.now();
    const result = this.db.hashJoinI64(t1, c1, t2, c2);
    const elapsed = performance.now() - t0;

    if (result.length !== JOIN_COUNT * 2) throw new Error(`join result length ${result.length} !== ${JOIN_COUNT * 2}`);
    for (let i = 0; i < JOIN_COUNT; i++) {
      if (Number(result[i * 2]) !== i) throw new Error(`join t1 id mismatch at ${i}: ${result[i * 2]} !== ${i}`);
      if (Number(result[i * 2 + 1]) !== i) throw new Error(`join t2 id mismatch at ${i}: ${result[i * 2 + 1]} !== ${i}`);
    }
    return elapsed;
  }

  joinColumnar(t1: string, c1: string, t2: string, c2: string) {
    return this.join(t1, c1, t2, c2);
  }
}

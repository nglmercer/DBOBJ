const { Database: DBOBJ, DataType } = require("../../index.node") as typeof import("../../index.d.ts");
import { TestSuite } from "../interface";
import { JOIN_COUNT } from "../constants";

export class DBOBJDirectSuite implements TestSuite {
  name = "DBOBJ Direct (API)";
  db = new DBOBJ("direct");

  insert(count: number) {
    this.db.createTable("users", [
      { name: "id", dataType: DataType.Integer, nullable: false },
      { name: "val", dataType: DataType.Integer },
    ]);
    this.db.createTable("names", [
      { name: "name", dataType: DataType.String },
    ]);
    this.db.createTable("actives", [
      { name: "active", dataType: DataType.Boolean },
    ]);

    const intBatch = new BigInt64Array(count * 2);
    const strBatch = new Array<string>(count);
    const boolBatch = new Array<boolean>(count);
    for (let i = 0; i < count; i++) {
      intBatch[i * 2] = BigInt(i);
      intBatch[i * 2 + 1] = BigInt(i * 10);
      strBatch[i] = `user_${i}`;
      boolBatch[i] = i % 2 === 0;
    }
    const t0 = performance.now();
    this.db.insertBatchI64("users", intBatch, 2);
    this.db.insertBatchString("names", strBatch, 1);
    this.db.insertBatchBool("actives", boolBatch, 1);
    return performance.now() - t0;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    this.db.getColumnI64(tableName, colName);
    return performance.now() - t0;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    this.db.findByI64(tableName, colName, value);
    return performance.now() - t0;
  }

  update(tableName: string, count: number) {
    const t0 = performance.now();
    for (let i = 0; i < count; i++) {
      this.db.updateRowI64(tableName, i, [i, i * 20]);
    }
    return performance.now() - t0;
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
    this.db.hashJoinI64(t1, c1, t2, c2);
    return performance.now() - t0;
  }
}

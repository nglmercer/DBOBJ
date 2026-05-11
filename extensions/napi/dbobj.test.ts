import { expect, test, describe } from "bun:test";
const { Database, DataType } = require("./index.js") as typeof import("./index.d.ts");

describe("DBOBJ N-API Bindings - Full Operations", () => {

  // ── BASIC CRUD ─────────────────────────────────────────────────────

  test("CRUD Operations", () => {
    const db = new Database("CRUD_Test");
    db.createTable("users", [
      { name: "age", dataType: DataType.Integer },
    ]);

    db.insertRowI64("users", [25]);
    db.insertRowI64("users", [30]);

    let ages = db.getColumnI64("users", "age");
    expect(ages.length).toBe(2);
    expect(ages[0]).toBe(25n);
    expect(ages[1]).toBe(30n);

    db.updateRowI64("users", 0, [35]);
    ages = db.getColumnI64("users", "age");
    expect(ages[0]).toBe(35n);

    const found = db.findByI64("users", "age", 35);
    expect(found.length).toBe(1);
    expect(found[0]).toBe(0n);

    db.deleteRow("users", 0);
    ages = db.getColumnI64("users", "age");
    expect(ages.length).toBe(1);
    expect(ages[0]).toBe(30n);
  });

  // ── GENERIC INSERT / UPDATE ────────────────────────────────────────

  test("Generic insertRow with mixed types", () => {
    const db = new Database("Test_GenInsert");
    db.createTable("t", [
      { name: "id", dataType: DataType.Integer },
      { name: "name", dataType: DataType.String },
      { name: "active", dataType: DataType.Boolean },
    ]);
    db.insertRow("t", [1, "alice", true]);
    db.insertRow("t", [2, "bob", false]);

    const rows = db.getRows("t");
    expect(rows.length).toBe(2);
    expect(rows[0].id).toBe(1);
    expect(rows[0].name).toBe("alice");
    expect(rows[0].active).toBe(true);
    expect(rows[1].id).toBe(2);
    expect(rows[1].name).toBe("bob");
    expect(rows[1].active).toBe(false);
  });

  test("Generic updateRow with mixed types", () => {
    const db = new Database("Test_GenUpdate");
    db.createTable("t", [
      { name: "id", dataType: DataType.Integer },
      { name: "name", dataType: DataType.String },
      { name: "val", dataType: DataType.Integer },
    ]);
    db.insertRow("t", [1, "alice", 100]);
    db.updateRow("t", 0, [1, "alice_updated", 200]);
    const rows = db.getRows("t");
    expect(rows.length).toBe(1);
    expect(rows[0].name).toBe("alice_updated");
    expect(rows[0].val).toBe(200);
  });

  test("Generic insertBatch with mixed types", () => {
    const db = new Database("Test_GenBatch");
    db.createTable("t", [
      { name: "id", dataType: DataType.Integer },
      { name: "name", dataType: DataType.String },
    ]);
    db.insertBatch("t", [1, "a", 2, "b", 3, "c"], 2);
    const rows = db.getRows("t");
    expect(rows.length).toBe(3);
    expect(rows[0].id).toBe(1);
    expect(rows[0].name).toBe("a");
    expect(rows[2].name).toBe("c");
  });

  // ── TYPED INSERT / UPDATE ──────────────────────────────────────────

  test("Typed Insert/Update methods", () => {
    const db = new Database("Typed_Test");

    db.createTable("strings", [{ name: "val", dataType: DataType.String }]);
    db.insertRowString("strings", ["hello"]);
    db.insertRowString("strings", ["world"]);
    db.updateRowString("strings", 0, ["hi"]);
    let rows = db.getRows("strings");
    expect(rows.length).toBe(2);
    expect(rows[0].val).toBe("hi");
    expect(rows[1].val).toBe("world");

    db.createTable("bools", [{ name: "val", dataType: DataType.Boolean }]);
    db.insertRowBool("bools", [true]);
    db.insertRowBool("bools", [false]);
    db.updateRowBool("bools", 0, [false]);
    rows = db.getRows("bools");
    expect(rows.length).toBe(2);
    expect(rows[0].val).toBe(false);
    expect(rows[1].val).toBe(false);

    db.createTable("batch_str", [
      { name: "a", dataType: DataType.String },
      { name: "b", dataType: DataType.String },
    ]);
    db.insertBatchString("batch_str", ["x", "y", "z", "w"], 2);
    rows = db.getRows("batch_str");
    expect(rows.length).toBe(2);
    expect(rows[0].a).toBe("x");
    expect(rows[1].b).toBe("w");

    db.createTable("batch_bool", [
      { name: "a", dataType: DataType.Boolean },
      { name: "b", dataType: DataType.Boolean },
    ]);
    db.insertBatchBool("batch_bool", [true, false, false, true], 2);
    rows = db.getRows("batch_bool");
    expect(rows.length).toBe(2);
    expect(rows[0].a).toBe(true);
    expect(rows[1].b).toBe(true);
  });

  test("insertBatchI64", () => {
    const db = new Database("Test_BatchI64");
    db.createTable("t", [
      { name: "a", dataType: DataType.Integer },
      { name: "b", dataType: DataType.Integer },
    ]);
    const batch = new BigInt64Array([BigInt(10), BigInt(100), BigInt(20), BigInt(200)]);
    db.insertBatchI64("t", batch, 2);
    const rows = db.getRows("t");
    expect(rows.length).toBe(2);
    expect(rows[0].a).toBe(10);
    expect(rows[0].b).toBe(100);
    expect(rows[1].a).toBe(20);
    expect(rows[1].b).toBe(200);
  });

  // ── TYPED READ / FIND ──────────────────────────────────────────────

  test("getColumnString / getColumnBool", () => {
    const db = new Database("Test_ColStrBool");
    db.createTable("s", [{ name: "v", dataType: DataType.String }]);
    db.createTable("b", [{ name: "v", dataType: DataType.Boolean }]);
    db.insertRowString("s", ["a"]);
    db.insertRowString("s", ["b"]);
    db.insertRowBool("b", [true]);
    db.insertRowBool("b", [false]);

    const strs = db.getColumnString("s", "v");
    expect(strs.length).toBe(2);
    expect(strs[0]).toBe("a");
    expect(strs[1]).toBe("b");

    const bools = db.getColumnBool("b", "v");
    expect(bools.length).toBe(2);
    expect(bools[0]).toBe(true);
    expect(bools[1]).toBe(false);
  });

  test("findByString / findByBool", () => {
    const db = new Database("Test_FindStrBool");
    db.createTable("s", [{ name: "v", dataType: DataType.String }]);
    db.createTable("b", [{ name: "v", dataType: DataType.Boolean }]);
    db.insertRowString("s", ["alpha"]);
    db.insertRowString("s", ["beta"]);
    db.insertRowString("s", ["alpha"]);
    db.insertRowBool("b", [true]);
    db.insertRowBool("b", [false]);
    db.insertRowBool("b", [true]);

    const foundStr = db.findByString("s", "v", "alpha");
    expect(foundStr.length).toBe(2);
    const foundBool = db.findByBool("b", "v", true);
    expect(foundBool.length).toBe(2);
  });

  // ── HASH JOIN ──────────────────────────────────────────────────────

  test("Hash Join", () => {
    const db = new Database("Join_Test");
    db.createTable("t1", [{ name: "val", dataType: DataType.Integer }]);
    db.createTable("t2", [{ name: "val", dataType: DataType.Integer }]);

    db.insertRowI64("t1", [10]); // ID 0
    db.insertRowI64("t2", [10]); // ID 0

    const joinResult = db.hashJoinI64("t1", "val", "t2", "val");
    expect(joinResult.length).toBe(2);
    expect(joinResult[0]).toBe(0n);
    expect(joinResult[1]).toBe(0n);
  });

  // ── SQL ────────────────────────────────────────────────────────────

  test("SQL Execution", () => {
    const db = new Database("SQL_Test");
    db.executeSql("CREATE TABLE users (id INTEGER, name STRING)");
    db.executeSql("INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob')");

    const result = db.executeSql("SELECT * FROM users WHERE id = 1");
    expect(Array.isArray(result)).toBe(true);
    expect(result.length).toBe(1);
    expect(result[0].name).toBe("Alice");
  });

  test("queryI64", () => {
    const db = new Database("Test_QueryI64");
    db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
    db.executeSql("INSERT INTO t (id, val) VALUES (1, 10), (2, 20)");

    const col = db.queryI64("SELECT val FROM t");
    expect(col.length).toBe(2);
    expect(col[0]).toBe(10n);
    expect(col[1]).toBe(20n);
  });

  test("queryJoinI64 via hashJoinI64", () => {
    const db = new Database("Test_QueryJoin");
    db.createTable("t", [{ name: "val", dataType: DataType.Integer }]);
    db.createTable("t2", [{ name: "score", dataType: DataType.Integer }]);
    db.insertRowI64("t", [10]);
    db.insertRowI64("t2", [10]);

    const joined = db.hashJoinI64("t", "val", "t2", "score");
    expect(joined.length).toBe(2);
  });

  // ── PREPARED STATEMENT ─────────────────────────────────────────────

  test("PreparedStatement run", () => {
    const db = new Database("Test_PrepRun");
    db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
    const stmt = db.prepare("INSERT INTO t (id, val) VALUES (?, ?)");
    stmt.run([1, 100]);
    stmt.run([2, 200]);
    const rows = db.getRows("t");
    expect(rows.length).toBe(2);
    expect(rows[0].val).toBe(100);
  });

  test("PreparedStatement allI64", () => {
    const db = new Database("Test_PrepAll");
    db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
    db.executeSql("INSERT INTO t (id, val) VALUES (1, 10), (2, 20)");
    const stmt = db.prepare("SELECT val FROM t");
    const col = stmt.allI64([]);
    expect(col.length).toBe(2);
    expect(col[0]).toBe(10n);
    expect(col[1]).toBe(20n);
  });

  test("PreparedStatement runBatch", () => {
    const db = new Database("Test_PrepBatch");
    db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
    const stmt = db.prepare("INSERT INTO t (id, val) VALUES (?, ?)");
    stmt.runBatch([[1, 10], [2, 20], [3, 30]]);
    const rows = db.getRows("t");
    expect(rows.length).toBe(3);
    expect(rows[2].val).toBe(30);
  });

  test("PreparedStatement runBatchI64", () => {
    const db = new Database("Test_PrepBatchI64");
    db.executeSql("CREATE TABLE t (id INTEGER, val INTEGER)");
    const stmt = db.prepare("INSERT INTO t (id, val) VALUES (?, ?)");
    const batch = new BigInt64Array([BigInt(1), BigInt(100), BigInt(2), BigInt(200), BigInt(3), BigInt(300)]);
    stmt.runBatchI64(batch, 2);
    const rows = db.getRows("t");
    expect(rows.length).toBe(3);
    expect(rows[1].val).toBe(200);
  });

  test("PreparedStatement runBatchValues", () => {
    const db = new Database("Test_PrepBatchVal");
    db.executeSql("CREATE TABLE t (id INTEGER, name STRING)");
    const stmt = db.prepare("INSERT INTO t (id, name) VALUES (?, ?)");
    stmt.runBatchValues([1, "a", 2, "b"], 2);
    const rows = db.getRows("t");
    expect(rows.length).toBe(2);
    expect(rows[0].name).toBe("a");
    expect(rows[1].name).toBe("b");
  });

  // ── META ───────────────────────────────────────────────────────────

  test("createIndex / createUniqueIndex", () => {
    const db = new Database("Test_Index");
    db.createTable("t", [
      { name: "id", dataType: DataType.Integer, nullable: false },
      { name: "val", dataType: DataType.Integer },
    ]);
    db.createIndex("t", "val");
    db.createUniqueIndex("t", "id");
    db.insertRowI64("t", [1, 10]);
    db.insertRowI64("t", [2, 20]);
  });

  test("getTableMetadata", () => {
    const db = new Database("Test_Meta");
    db.createTable("t", [
      { name: "id", dataType: DataType.Integer },
      { name: "name", dataType: DataType.String },
    ]);
    const meta = db.getTableMetadata("t");
    expect(meta).not.toBeNull();
    expect(meta!.name).toBe("t");
    expect(meta!.columnCount).toBe(2);
    expect(meta!.rowCount).toBe(0);

    db.insertRow("t", [1, "a"]);
    const meta2 = db.getTableMetadata("t");
    expect(meta2!.rowCount).toBe(1);
  });

  test("listTables", () => {
    const db = new Database("Test_List");
    db.createTable("a", [{ name: "x", dataType: DataType.Integer }]);
    db.createTable("b", [{ name: "y", dataType: DataType.Integer }]);
    const tables = db.listTables();
    expect(tables.length).toBe(2);
    expect(tables).toContain("a");
    expect(tables).toContain("b");
  });

  test("save / load roundtrip", () => {
    const path = `Test_Save_${Date.now()}.dbobj`;
    const db = new Database(path);
    db.createTable("t", [{ name: "val", dataType: DataType.Integer }]);
    db.insertRowI64("t", [42]);
    db.save(path);

    const loaded = Database.load(path);
    const meta = loaded.getTableMetadata("t");
    expect(meta).not.toBeNull();
    expect(meta!.rowCount).toBe(1);

    const col = loaded.getColumnI64("t", "val");
    expect(col[0]).toBe(42n);
  });

  // ── FLOAT / BLOB ───────────────────────────────────────────────────

  test("Float column via insertRow", () => {
    const db = new Database("Test_Float");
    db.createTable("t", [
      { name: "val", dataType: DataType.Float },
    ]);
    db.insertRow("t", [3.14]);
    const rows = db.getRows("t");
    expect(rows.length).toBe(1);
    expect(typeof rows[0].val).toBe("number");
  });

  test("Blob column via insertRow", () => {
    const db = new Database("Test_Blob");
    db.createTable("t", [
      { name: "val", dataType: DataType.Blob },
    ]);
    // serde_json::Value represents blobs as arrays of numbers
    db.insertRow("t", [JSON.stringify([1, 2, 3])]);
    const rows = db.getRows("t");
    expect(rows.length).toBe(1);
    expect(rows[0].val).toBeString();
  });

  // ── NULLABLE ───────────────────────────────────────────────────────

  test("Nullable columns", () => {
    const db = new Database("Test_Nullable");
    db.createTable("t", [
      { name: "id", dataType: DataType.Integer, nullable: false },
      { name: "name", dataType: DataType.String },
    ]);
    db.insertRow("t", [1, null]);
    const rows = db.getRows("t");
    expect(rows.length).toBe(1);
    expect(rows[0].name).toBeNull();
  });

  // ── ERROR MESSAGES ─────────────────────────────────────────────────

  test("Schema violation error shows column list", () => {
    const db = new Database("Test_ErrMsg");
    db.createTable("users", [
      { name: "id", dataType: DataType.Integer },
      { name: "name", dataType: DataType.String },
    ]);
    try {
      db.insertRow("users", [1]);
      expect.unreachable();
    } catch (e: any) {
      expect(e.message).toContain("users");
      expect(e.message).toContain("id (Integer)");
      expect(e.message).toContain("name (String)");
      expect(e.message).toContain("insert_batch_values");
    }
  });
});

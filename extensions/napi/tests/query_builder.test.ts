import { expect, test } from "bun:test";
const { Database } = require("../index.js") as typeof import("../index.d.ts");

test("QueryBuilder select all", () => {
  const db = new Database("QB_Test_Select");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25), (3, 'Charlie', 35)");

  const qb = db.createQueryBuilder();
  const rows = qb.select("users").execute() as Array<any>;
  expect(rows.length).toBe(3);
});

test("QueryBuilder select with whereEq", () => {
  const db = new Database("QB_Test_WhereEq");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25)");

  const qb = db.createQueryBuilder();
  const rows = qb.select("users").whereEq("name", "Alice").execute() as Array<any>;
  expect(rows.length).toBe(1);
  expect(rows[0].name).toBe("Alice");
});

test("QueryBuilder select with whereGt and orderBy", () => {
  const db = new Database("QB_Test_Order");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25), (3, 'Charlie', 35)");

  const qb = db.createQueryBuilder();
  const rows = qb.select("users").whereGt("age", 28).orderBy("age", false).execute() as Array<any>;
  expect(rows.length).toBe(2);
  expect(rows[0].age).toBe(30);
  expect(rows[1].age).toBe(35);
});

test("QueryBuilder select with limit and offset", () => {
  const db = new Database("QB_Test_Limit");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25), (3, 'Charlie', 35), (4, 'Diana', 28)");

  const qb = db.createQueryBuilder();
  const rows = qb.select("users").orderBy("age", false).limit(2).offset(1).execute() as Array<any>;
  expect(rows.length).toBe(2);
});

test("QueryBuilder select with columns projection", () => {
  const db = new Database("QB_Test_Columns");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)");

  const qb = db.createQueryBuilder();
  const rows = qb.select("users").columns(["name"]).whereEq("age", 30).execute() as Array<any>;
  expect(rows.length).toBe(1);
});

test("QueryBuilder select with whereLike", () => {
  const db = new Database("QB_Test_Like");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25)");

  const qb = db.createQueryBuilder();
  const rows = qb.select("users").whereLike("name", "%lice").execute() as Array<any>;
  expect(rows.length).toBe(1);
  expect(rows[0].name).toBe("Alice");
});

test("QueryBuilder select with chained where conditions", () => {
  const db = new Database("QB_Test_Chained");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25), (3, 'Charlie', 35)");

  const qb = db.createQueryBuilder();
  const rows = qb.select("users").whereGt("age", 25).whereLt("age", 35).execute() as Array<any>;
  expect(rows.length).toBe(1);
});

test("QueryBuilder first() returns first matching row", () => {
  const db = new Database("QB_Test_First");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25)");

  const qb = db.createQueryBuilder();
  const row = qb.select("users").whereEq("name", "Bob").first();
  expect(row).not.toBeNull();
  expect(row!.name).toBe("Bob");
});

test("QueryBuilder first() returns null when no match", () => {
  const db = new Database("QB_Test_First_None");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)");

  const qb = db.createQueryBuilder();
  const row = qb.select("users").whereEq("name", "Nobody").first();
  expect(row).toBeNull();
});

test("QueryBuilder insert", () => {
  const db = new Database("QB_Test_Insert");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");

  const qb = db.createQueryBuilder();
  const rows = qb.insert("users").set("name", "Alice").set("age", 30).execute() as Array<any>;
  expect(rows.length).toBe(1);
  expect(rows[0].name).toBe("Alice");
  expect(rows[0].age).toBe(30);

  const all = db.createQueryBuilder().select("users").execute() as Array<any>;
  expect(all.length).toBe(1);
});

test("QueryBuilder update", () => {
  const db = new Database("QB_Test_Update");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25)");

  const qb = db.createQueryBuilder();
  const updated = qb.update("users").set("age", 31).whereEq("name", "Alice").execute() as Array<any>;
  expect(updated.length).toBe(1);
  expect(updated[0].age).toBe(31);

  const row = db.createQueryBuilder().select("users").whereEq("name", "Alice").first();
  expect(row!.age).toBe(31);
});

test("QueryBuilder delete", () => {
  const db = new Database("QB_Test_Delete");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING, age INTEGER)");
  db.executeSql("INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30), (2, 'Bob', 25)");

  const qb = db.createQueryBuilder();
  const deleted = qb.delete("users").whereEq("name", "Bob").execute() as Array<any>;
  expect(deleted.length).toBe(1);

  const all = db.createQueryBuilder().select("users").execute() as Array<any>;
  expect(all.length).toBe(1);
});

test("QueryBuilder join", () => {
  const db = new Database("QB_Test_Join");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING)");
  db.executeSql("CREATE TABLE scores (id INTEGER, score INTEGER)");
  db.executeSql("INSERT INTO users (id, name) VALUES (0, 'Alice'), (1, 'Bob'), (2, 'Charlie')");
  db.executeSql("INSERT INTO scores (id, score) VALUES (0, 95), (1, 87), (2, 92)");

  const qb = db.createQueryBuilder();
  const rows = qb.select("users").join("scores", "id", "id").execute() as Array<any>;
  expect(rows.length).toBe(3);
  expect(rows[0].name).toBe("Alice");

  // Verify join with WHERE condition
  const filtered = db.createQueryBuilder()
    .select("users")
    .join("scores", "id", "id")
    .whereEq("name", "Bob")
    .execute() as Array<any>;
  expect(filtered.length).toBe(1);
  expect(filtered[0].name).toBe("Bob");
});

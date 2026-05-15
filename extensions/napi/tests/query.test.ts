import { expect, test } from "bun:test";
const { Database } = require("../index.js") as typeof import("../index.d.ts");

test("query() and PreparedStatement all()/get()", () => {
  const db = new Database("Query_Test");
  db.executeSql("CREATE TABLE users (id INTEGER, name STRING)");
  db.executeSql("INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob')");

  const stmt = db.query("SELECT * FROM users WHERE id = ?");

  const alice = stmt.get([1]);
  expect(alice).not.toBeNull();
  expect(alice.name).toBe("Alice");

  const bob = stmt.all([2]);
  expect(Array.isArray(bob)).toBe(true);
  expect(bob.length).toBe(1);
  expect(bob[0].name).toBe("Bob");

  const none = stmt.get([3]);
  expect(none).toBeNull();
});

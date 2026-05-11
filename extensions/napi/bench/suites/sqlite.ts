import { Database as SQLite } from "bun:sqlite";
import { TestSuite } from "../interface";
import { JOIN_COUNT } from "../constants";

export class BunSQLiteSuite implements TestSuite {
  name = "Bun SQLite (Native)";
  db = new SQLite(":memory:");

  insert(count: number) {
    this.db.run("CREATE TABLE users (id INTEGER, val INTEGER)");
    this.db.run("CREATE TABLE names (name TEXT)");
    this.db.run("CREATE TABLE actives (active INTEGER)");
    this.db.run("CREATE INDEX idx_id ON users (id)");

    const intStmt = this.db.prepare("INSERT INTO users (id, val) VALUES (?, ?)");
    const strStmt = this.db.prepare("INSERT INTO names (name) VALUES (?)");
    const boolStmt = this.db.prepare("INSERT INTO actives (active) VALUES (?)");

    const t0 = performance.now();
    this.db.transaction(() => {
      for (let i = 0; i < count; i++) {
        intStmt.run(i, i * 10);
        strStmt.run(`user_${i}`);
        boolStmt.run(i % 2 === 0 ? 1 : 0);
      }
    })();
    return performance.now() - t0;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    this.db.prepare(`SELECT ${colName} FROM ${tableName}`).values();
    return performance.now() - t0;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    this.db.prepare(`SELECT * FROM ${tableName} WHERE ${colName} = ?`).get(value);
    return performance.now() - t0;
  }

  update(tableName: string, count: number) {
    const stmt = this.db.prepare(`UPDATE ${tableName} SET val = ? WHERE id = ?`);
    const t0 = performance.now();
    this.db.transaction(() => {
      for (let i = 0; i < count; i++) stmt.run(i * 20, i);
    })();
    return performance.now() - t0;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.run(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
    this.db.run(`CREATE INDEX idx_stats_id ON ${t2} (id)`);
    const stmt = this.db.prepare(`INSERT INTO ${t2} (id, score) VALUES (?, ?)`);
    this.db.transaction(() => {
      for (let i = 0; i < JOIN_COUNT; i++) stmt.run(i, i + 5);
    })();

    const t0 = performance.now();
    this.db.query(`SELECT * FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`).all();
    return performance.now() - t0;
  }
}

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
    const elapsed = performance.now() - t0;

    const usersCount = (this.db.prepare("SELECT COUNT(*) as c FROM users").get() as any).c;
    const namesCount = (this.db.prepare("SELECT COUNT(*) as c FROM names").get() as any).c;
    const activesCount = (this.db.prepare("SELECT COUNT(*) as c FROM actives").get() as any).c;
    if (usersCount !== count) throw new Error(`users count ${usersCount} !== ${count}`);
    if (namesCount !== count) throw new Error(`names count ${namesCount} !== ${count}`);
    if (activesCount !== count) throw new Error(`actives count ${activesCount} !== ${count}`);

    return elapsed;
  }

  readColumn(tableName: string, colName: string) {
    const t0 = performance.now();
    const vals = this.db.prepare(`SELECT ${colName} FROM ${tableName}`).values();
    const elapsed = performance.now() - t0;
    if (vals.length === 0) throw new Error(`column ${colName} is empty`);
    return elapsed;
  }

  find(tableName: string, colName: string, value: any) {
    const t0 = performance.now();
    const row = this.db.prepare(`SELECT * FROM ${tableName} WHERE ${colName} = ?`).get(value) as any;
    const elapsed = performance.now() - t0;
    if (!row) throw new Error(`find ${colName}=${value} returned empty`);
    if (Number(row.val) !== value * 10) throw new Error(`find val mismatch: ${row.val} !== ${value * 10}`);
    return elapsed;
  }

  update(tableName: string, count: number) {
    const stmt = this.db.prepare(`UPDATE ${tableName} SET val = ? WHERE id = ?`);
    const t0 = performance.now();
    this.db.transaction(() => {
      for (let i = 0; i < count; i++) stmt.run(i * 20, i);
    })();
    const elapsed = performance.now() - t0;

    const row = this.db.prepare("SELECT val FROM users WHERE id = 50").get() as any;
    if (row.val !== 1000) throw new Error(`update val mismatch: ${row.val} !== 1000`);
    return elapsed;
  }

  join(t1: string, c1: string, t2: string, c2: string) {
    this.db.run(`CREATE TABLE ${t2} (id INTEGER, score INTEGER)`);
    this.db.run(`CREATE INDEX idx_stats_id ON ${t2} (id)`);
    const stmt = this.db.prepare(`INSERT INTO ${t2} (id, score) VALUES (?, ?)`);
    this.db.transaction(() => {
      for (let i = 0; i < JOIN_COUNT; i++) stmt.run(i, i + 5);
    })();

    const t0 = performance.now();
    const rows = this.db.query(`SELECT * FROM ${t1} INNER JOIN ${t2} ON ${t1}.id = ${t2}.id`).all() as Array<Record<string, any>>;
    const elapsed = performance.now() - t0;

    if (rows.length !== JOIN_COUNT) throw new Error(`join result count ${rows.length} !== ${JOIN_COUNT}`);
    if (Number(rows[0].val) !== 0) throw new Error(`join val mismatch: ${rows[0].val} !== 0`);
    return elapsed;
  }
}

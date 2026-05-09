use criterion::{Criterion, criterion_group, criterion_main};
use dbobj::sql::local_parser::Parser as LocalParser;
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser as SqlParser;
use std::time::Duration;

fn bench_create_table(c: &mut Criterion) {
    let sql = "CREATE TABLE users (id INTEGER, name TEXT, age INTEGER, email TEXT)";
    let mut group = c.benchmark_group("parse_create_table");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            let mut parser = LocalParser::new(sql);
            parser.parse_statements().unwrap();
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
        });
    });
    group.finish();
}

fn bench_insert_single(c: &mut Criterion) {
    let sql = "INSERT INTO users (id, name, age) VALUES (1, 'Alice', 30)";
    let mut group = c.benchmark_group("parse_insert_single");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            let mut parser = LocalParser::new(sql);
            parser.parse_statements().unwrap();
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
        });
    });
    group.finish();
}

fn bench_insert_multi(c: &mut Criterion) {
    let sql = "INSERT INTO users (name, age) VALUES ('Alice', 30), ('Bob', 25), ('Carol', 35), ('Dave', 40), ('Eve', 28)";
    let mut group = c.benchmark_group("parse_insert_multi");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            let mut parser = LocalParser::new(sql);
            parser.parse_statements().unwrap();
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
        });
    });
    group.finish();
}

fn bench_select_simple(c: &mut Criterion) {
    let sql = "SELECT * FROM users WHERE id = 1";
    let mut group = c.benchmark_group("parse_select_simple");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            let mut parser = LocalParser::new(sql);
            parser.parse_statements().unwrap();
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
        });
    });
    group.finish();
}

fn bench_select_complex_where(c: &mut Criterion) {
    let sql = "SELECT * FROM t WHERE a > 1 AND b = 'x' OR c < 5 AND d >= 10";
    let mut group = c.benchmark_group("parse_select_complex_where");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            let mut parser = LocalParser::new(sql);
            parser.parse_statements().unwrap();
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
        });
    });
    group.finish();
}

fn bench_update(c: &mut Criterion) {
    let sql = "UPDATE users SET age = 31, name = 'Bob' WHERE id = 1";
    let mut group = c.benchmark_group("parse_update");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            let mut parser = LocalParser::new(sql);
            parser.parse_statements().unwrap();
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
        });
    });
    group.finish();
}

fn bench_delete(c: &mut Criterion) {
    let sql = "DELETE FROM users WHERE id = 5";
    let mut group = c.benchmark_group("parse_delete");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            let mut parser = LocalParser::new(sql);
            parser.parse_statements().unwrap();
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
        });
    });
    group.finish();
}

fn bench_join(c: &mut Criterion) {
    let sql = "SELECT * FROM users INNER JOIN orders ON users.user_id = orders.user_id";
    let mut group = c.benchmark_group("parse_join");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            let mut parser = LocalParser::new(sql);
            parser.parse_statements().unwrap();
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
        });
    });
    group.finish();
}

fn bench_batch(c: &mut Criterion) {
    let sqls = [
        "CREATE TABLE t (id INT, val TEXT)",
        "INSERT INTO t VALUES (1, 'a')",
        "INSERT INTO t VALUES (2, 'b'), (3, 'c')",
        "SELECT * FROM t WHERE id = 1",
        "SELECT * FROM t WHERE val = 'x' AND id > 0 OR val = 'y'",
        "UPDATE t SET val = 'z' WHERE id = 2",
        "DELETE FROM t WHERE id = 3",
        "ALTER TABLE t ADD COLUMN extra TEXT",
        "SELECT * FROM t INNER JOIN u ON t.id = u.t_id",
        "INSERT INTO t (id, val) VALUES (?, ?)",
    ];
    let mut group = c.benchmark_group("parse_batch_10");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(3));
    group.bench_function("LocalParser", |b| {
        b.iter(|| {
            for sql in &sqls {
                let mut parser = LocalParser::new(sql);
                parser.parse_statements().unwrap();
            }
        });
    });
    group.bench_function("sqlparser", |b| {
        b.iter(|| {
            for sql in &sqls {
                SqlParser::parse_sql(&SQLiteDialect {}, sql).unwrap();
            }
        });
    });
    group.finish();
}

criterion_group!(
    parser_benches,
    bench_create_table,
    bench_insert_single,
    bench_insert_multi,
    bench_select_simple,
    bench_select_complex_where,
    bench_update,
    bench_delete,
    bench_join,
    bench_batch,
);
criterion_main!(parser_benches);

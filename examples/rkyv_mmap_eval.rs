use dbobj::core::{Id, Value};
use memmap2::Mmap;
use rkyv::{Archive, Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::time::Instant;

// Simplified types for Rkyv evaluation
#[derive(Archive, Deserialize, Serialize, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvalRow {
    pub id: u64,
    pub values: Vec<Value>,
}

#[derive(Archive, Deserialize, Serialize, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvalTable {
    pub name: String,
    pub rows: Vec<EvalRow>,
}

#[derive(Archive, Deserialize, Serialize, Debug, serde::Serialize, serde::Deserialize)]
pub struct EvalDatabase {
    pub name: String,
    pub tables: Vec<EvalTable>,
}

fn main() {
    println!("--- Rkyv + Mmap Performance Evaluation ---");

    let num_rows = 100_000;
    let mut rows = Vec::with_capacity(num_rows);
    for i in 0..num_rows {
        rows.push(EvalRow {
            id: i as u64,
            values: vec![
                Value::from(i as i64),
                Value::from(format!("user_{}", i)),
                Value::from(i % 2 == 0),
            ],
        });
    }

    let db = EvalDatabase {
        name: "EvalDB".to_string(),
        tables: vec![EvalTable {
            name: "users".to_string(),
            rows,
        }],
    };

    // --- 1. Bitcode Serialization (Current baseline) ---
    println!("\n[Bitcode Baseline]");
    let start = Instant::now();
    let bitcode_bytes = bitcode::serialize(&db).unwrap();
    println!("Bitcode Serialize: {:?}", start.elapsed());
    println!("Bitcode Size: {} bytes", bitcode_bytes.len());

    let start = Instant::now();
    let _: EvalDatabase = bitcode::deserialize(&bitcode_bytes).unwrap();
    println!("Bitcode Deserialize: {:?}", start.elapsed());

    // --- 2. Rkyv Serialization ---
    println!("\n[Rkyv + Standard File I/O]");
    let start = Instant::now();
    let rkyv_bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&db).unwrap();
    println!("Rkyv Serialize: {:?}", start.elapsed());
    println!("Rkyv Size: {} bytes", rkyv_bytes.len());

    // Save to disk for mmap test
    let path = "eval_rkyv.db";
    File::create(path).unwrap().write_all(&rkyv_bytes).unwrap();

    let start = Instant::now();
    // In Rkyv, "deserialization" is just accessing the bytes.
    let archived = rkyv::access::<ArchivedEvalDatabase, rkyv::rancor::Error>(&rkyv_bytes).unwrap();
    println!("Rkyv Access (Check Root): {:?}", start.elapsed());
    println!("Sample Data: id={}", archived.tables[0].rows[500].id);

    // --- 3. Rkyv + Mmap ---
    println!("\n[Rkyv + Mmap]");
    let file = File::open(path).unwrap();
    let mmap = unsafe { Mmap::map(&file).unwrap() };

    let start = Instant::now();
    // Zero-copy access from mmap
    let archived_mmap = rkyv::access::<ArchivedEvalDatabase, rkyv::rancor::Error>(&mmap).unwrap();
    println!("Mmap Access Time: {:?}", start.elapsed());
    println!(
        "Mmap Sample Data: id={}",
        archived_mmap.tables[0].rows[99999].id
    );

    // Speed comparison of lookup
    let start = Instant::now();
    let mut sum = 0;
    for row in archived_mmap.tables[0].rows.iter() {
        if let rkyv::Archived::<Value>::Integer(i) = &row.values[0] {
            sum += i.to_native();
        }
    }
    println!(
        "Scan 100k rows (Archived): {:?} (sum={})",
        start.elapsed(),
        sum
    );

    std::fs::remove_file(path).ok();
}

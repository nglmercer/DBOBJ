# Benchmarks

## Rust Core (Criterion) — After Optimizations

Results from `cargo bench` (sample size: 10, measurement time: 3s).

### Core Operations

| Operation | DBOBJ (Ops/sec) | SQLite (Ops/sec) | Speedup |
| :--- | :--- | :--- | :--- |
| **Insert (Single)** | **~2,046,000** | ~356,000 | **5.7x** |
| **Insert (Batch 100)** | **~3,985,000** | ~1,980,000 | **2.0x** |
| **Insert (Batch Raw 100)** | **~7,163,000** | - | - |
| **Read (ID)** | **~6,815,000** | ~335,000 | **20.3x** |
| **Search (Scan)** | **~108,000** | ~11,000 | **9.8x** |
| **Search (Indexed)** | **~4,898,000** | ~304,000 | **16.1x** |
| **Hash Join (1k rows)** | **~4,034** | ~1,737 | **2.3x** |
| **Large Hash Join (100k rows)** | **~39.7** | ~14.0 | **2.8x** |

### Serialization (10,000 rows)

| Operation | Time | Ops/sec |
| :--- | :--- | :--- |
| **Bitcode Serialize** | ~390 µs | ~2,562 |
| **Bitcode Deserialize** | ~613 µs | ~1,631 |
| **Mmap Save (rkyv)** | ~5.91 ms | ~169 |
| **Mmap Load** | ~196 µs | ~5,097 |
| **Mmap Access (zero-copy)** | **~1.95 ns** | **~512,000,000** |
| **Mmap Deserialize** | ~1.27 ms | ~790 |

## End-to-End (100K rows, mixed types)

| Operation | Direct (API) | SQL Bulk | SQL Prep | Bun SQLite |
|-----------|-------------|----------|----------|------------|
| INSERT    | **~69ms**   | ~340ms   | ~144ms   | ~526ms     |
| READ      | **~0.6ms**  | ~19ms    | ~0.7ms   | ~26ms      |
| FIND      | **~0.02ms**  | ~0.08ms  | ~0.07ms  | ~0.21ms    |
| UPDATE    | **~8ms**    | ~66ms    | ~4ms     | ~18ms      |
| JOIN      | **~4ms**    | ~34ms    | ~5ms     | ~16ms      |

Run locally:

```bash
cd extensions/napi
bun run build
bun bench.ts
```

## Key Insights

- **Direct API** uses zero-copy `BigInt64Array` — avoids JSON serialization entirely
- **SQL Prepared** uses flat typed arrays (`runBatchI64`) — 2-3x faster than individual prepared statements
- **Column reads** are 60x faster than SQLite because they bypass row construction
- **Hash joins** avoid nested loop joins by building a hash table on the smaller table

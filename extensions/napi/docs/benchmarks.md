# Benchmarks

## Rust Core (Criterion) — After Optimizations

Results from `cargo bench` (sample size: 10, measurement time: 3s).

### Core Operations

| Operation | DBOBJ (Ops/sec) | SQLite (Ops/sec) | Speedup |
| :--- | :--- | :--- | :--- |
| **Insert (Single)** | **~1,327,000** | ~265,000 | **5.0x** |
| **Insert (Batch 100)** | **~2,873,000** | ~1,618,000 | **1.8x** |
| **Insert (Batch Raw 100)** | **~5,511,000** | - | - |
| **Read (ID)** | **~5,669,000** | ~275,000 | **20.6x** |
| **Search (Scan)** | **~100,000** | ~7,600 | **13.2x** |
| **Search (Indexed)** | **~4,021,000** | ~226,000 | **17.8x** |
| **Hash Join (1k rows)** | **~3,155** | ~1,277 | **2.5x** |
| **Large Hash Join (100k rows)** | **~30.6** | ~10.1 | **3.0x** |

### Serialization (10,000 rows)

| Operation | Time | Ops/sec |
| :--- | :--- | :--- |
| **Bitcode Serialize** | ~472 µs | ~2,118 |
| **Bitcode Deserialize** | ~722 µs | ~1,384 |
| **Mmap Save (rkyv)** | ~5.78 ms | ~173 |
| **Mmap Load** | ~284 µs | ~3,524 |
| **Mmap Access (zero-copy)** | **~2.44 ns** | **~409,000,000** |
| **Mmap Deserialize** | ~1.62 ms | ~616 |

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

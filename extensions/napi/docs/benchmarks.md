# Benchmarks

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

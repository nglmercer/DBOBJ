# DBOBJ Optimization Evaluation

This document evaluates potential optimizations for **DBOBJ** to further increase its performance, focusing particularly on advanced serialization techniques, alternative libraries, and architectural enhancements. 

## 1. Serialization & Deserialization Optimizations [x]

Currently, DBOBJ utilizes `bincode`, `postcard`, and `serde_json`. While `bincode` and `postcard` are fast, they still require an allocation and copying phase during deserialization. We can achieve massive performance gains by evaluating the following libraries:

### A. Zero-Copy Deserialization with `rkyv` (Highly Recommended) [x]
`rkyv` is widely considered the fastest serialization framework in the Rust ecosystem. 
- **How it works:** Instead of allocating memory and copying bytes into Rust structs, `rkyv` formats the serialized data so that it matches the memory layout of the structs. You can simply cast a byte buffer to the struct type and access it immediately.
- **Performance Impact:** Deserialization time drops to almost **O(1)** (effectively zero). Startup times for loading the database from disk would become virtually instantaneous, regardless of the database size.
- **Trade-off:** Requires deriving `Archive`, `Serialize`, and `Deserialize` on all types. Accessing archived types has a slightly different syntax than accessing native Rust types.
- **Status:** ✅ **Implemented.** All core types (`Value`, `Id`, `Table`, `Schema`, `DatabaseSnapshot`, `VersionLog`) derive `rkyv::Archive`, `rkyv::Serialize`, and `rkyv::Deserialize`. An `RkyvAdapter` exists in `src/storage/adapter.rs`.

### B. `bitcode` [x]
If zero-copy is too invasive for the codebase, `bitcode` is an extremely fast alternative to `bincode` and `postcard`.
- **How it works:** It uses bit-level packing and doesn't rely on `serde`'s data model, allowing it to bypass Serde's overhead.
- **Performance Impact:** Often results in smaller payload sizes than `bincode` and can be up to 2x-5x faster at serialization/deserialization.
- **Trade-off:** Requires its own `Encode` / `Decode` traits instead of standard `serde`.
- **Status:** ✅ **Implemented.** `BitcodeAdapter` is the current default storage adapter and benchmark baseline.

### C. `FlatBuffers` / `Cap'n Proto` (Evaluated — Not Recommended)
- **How it works:** Similar to `rkyv`, these are zero-copy formats. 
- **Performance Impact:** Extremely high performance for reading data.
- **Trade-off:** Requires writing schemas in an external IDL (Interface Definition Language) file and generating Rust code. It is less ergonomic for an embedded database that changes rapidly.
- **Evaluation Result:** ❌ **Not recommended for DBOBJ.**
  1. **Dynamic Schema Incompatibility:** DBOBJ is a dynamic embedded database where tables and schemas are defined at runtime (`Schema { columns: Vec<ColumnDefinition> }`). FlatBuffers requires static `.fbs` schemas compiled ahead of time with `flatc`. This is fundamentally incompatible with DBOBJ's architecture.
  2. **No Native HashMap Support:** FlatBuffers does not support `HashMap` / `BTreeMap` natively. DBOBJ's core data structure is `RowData = FastHashMap<CompactString, Value>`. Emulating maps in FlatBuffers requires vectors of key-value pairs with manual lookup logic, which is both slower and ergonomically poor.
  3. **Experimental Rust Support:** The official `flatbuffers` crate documentation states: *"At this time, Rust support is experimental, and APIs may change between minor versions."* Relying on an experimental API for core storage is risky.
  4. **Build Complexity:** Requires installing the `flatc` compiler and integrating code generation into the build pipeline. This adds friction for contributors and users.
  5. **rkyv is a Superior Alternative:** `rkyv` provides the same zero-copy benefits without any of FlatBuffers' drawbacks. It uses standard Rust derive macros, supports all Rust types (including `HashMap`), and has a mature, stable API.

**Conclusion:** Use `rkyv` instead. It delivers identical zero-copy performance with full compatibility with DBOBJ's dynamic, map-heavy data model.

---

## 2. Memory & Allocator Optimizations

Since DBOBJ is an in-memory database, memory allocation is likely a major bottleneck during heavy `INSERT` or `UPDATE` operations.

### A. Use a Custom Global Allocator (`mimalloc` or `jemalloc`)[x]
The default system allocator in Rust can struggle with high-concurrency allocations. 
- **Implementation:** Simply drop in `mimalloc` or `jemallocator` in the `Cargo.toml` and configure it in `main.rs`/`lib.rs`.
- **Impact:** Can improve general database throughput by 10-20% under concurrent workloads by reducing lock contention in the memory allocator.
- **Status:** ✅ **Implemented.** `mimalloc = "0.1.50"` is already in `Cargo.toml`.

### B. Memory-Mapped Files (`mmap`) 
Instead of reading the entire database file into memory via standard file I/O:
- **Implementation:** Use the `memmap2` crate. Map the `.db` file directly into memory and pair this with a zero-copy library like `rkyv`.
- **Impact:** The OS handles paging data in and out of RAM. This significantly reduces memory overhead, prevents Out-Of-Memory (OOM) crashes on large datasets, and enables instant database loading.
- **Status:** ✅ **Implemented.** `MmapStorage` in `src/storage/mmap.rs` provides:
  - `save`: serializes a `Database` via `rkyv` to disk.
  - `load`: memory-maps the file and validates the archived root.
  - `access`: returns a zero-copy `&ArchivedDatabaseSnapshot` in **~40 ns** (effectively instant).
  - `deserialize` / `load_database`: optional full copy back into mutable `Database`.

#### Design Notes
`MmapStorage` is kept **separate** from the existing `Storage<S: SerializerAdapter>` trait so that:
1. `BitcodeAdapter` and `RkyvAdapter` remain untouched for full mutability.
2. Users who need instant startup on massive datasets can opt-in to `MmapStorage`.
3. The archived view is immutable; mutations require a `deserialize` step, which rebuilds the owned `Database` and its indexes.

---

## 3. Data Structure & Engine Optimizations

### A. Perfect Hashing or NoHash for IDs
If `Id`s are largely sequential integers (or UUIDs that are already well-distributed):
- **Implementation:** Replace `ahash::RandomState` on ID lookups with a pass-through hasher like `nohash-hasher`.
- **Impact:** Bypasses the CPU overhead of calculating cryptographic/complex hashes for keys that are already unique identifiers.
- **Status:** ✅ **Partially implemented.** `NoHashHashMap` type alias exists in `src/core/mod.rs` but is not yet used for ID indexes.

### B. Global String Interning
You are currently using `compact_str::CompactString`, which is excellent for inline storage of short strings. However, for repeated strings across rows (e.g., categories, roles, tags):
- **Implementation:** Implement a global string intern pool (using crates like `lasso` or `string-interner`). Store an `u32` token in the `Row` instead of the actual string.
- **Impact:** Drastically reduces memory usage and speeds up string comparisons (comparing two `u32` integers is faster than comparing byte slices).
- **Status:** ⚠️ **Evaluated — integration blocked.** `examples/string_interner_eval.rs` measures the benefits and documents the blockers:
  1. **Global State:** `DefaultStringInterner` is not `Send/Sync` by default; sharing across threads requires `Arc<RwLock<...>>` or a thread-safe backend.
  2. **rkyv Incompatibility:** `string-interner` types do not implement `rkyv::Archive`. DBOBJ's zero-copy storage path would need custom archive impls or a separate serde/bitcode fallback.
  3. **API Breakage:** Replacing `Value::String(CompactString)` with `Value::InternedString(u32)` touches every insert, query, and display path.
  4. **Persistence Complexity:** Saving/loading requires serializing the intern pool alongside the rows.
  
  **Verdict:** Keep `CompactString` for now. Revisit interning only if a dedicated columnar/OLAP engine is added later, where immutable tokenized columns would be a natural fit.

### C. Columnar Storage (Apache Arrow)
If the database workloads shift toward OLAP (Analytical Queries - e.g., "Sum all ages", "Count users by city"):
- **Implementation:** Store data in columns rather than rows (Dense Row model).
- **Impact:** Allows the use of **SIMD (Single Instruction, Multiple Data)** instructions to process thousands of records simultaneously.

---

## Summary of Recommendations

To push DBOBJ's performance even further beyond SQLite:
1. **Explore `rkyv`** to eliminate deserialization costs entirely. ✅ Already implemented.
2. **Switch the global allocator** to `mimalloc` for a free 10-20% speedup on multi-threaded operations. ✅ Already implemented.
3. **Implement `mmap`** combined with `rkyv` for instant startup times. ✅ Implemented via `MmapStorage`.
4. **Do NOT pursue FlatBuffers.** `rkyv` is a strictly better fit for DBOBJ's dynamic schema and map-based data model.
5. **Defer string interning.** Evaluated; blocked by rkyv incompatibility and high API churn for limited gain on typical row-heavy workloads.

## 4. Evaluation Results: Rkyv + Mmap (100,000 rows)

We implemented an evaluation example (`examples/rkyv_mmap_eval.rs`) to compare our current `Bitcode` baseline against `rkyv` + `memmap2`.

| Metric | Bitcode (Baseline) | Rkyv + Mmap (Zero-Copy) |
| :--- | :--- | :--- |
| **Serialization** | **~8.1 ms** | ~12.6 ms |
| **Deserialization** | ~21.9 ms | **~0.0 ms (Instant)** |
| **Data Size** | **~1.9 MB** | ~7.9 MB |
| **Scan (100k rows)** | ~1.2 ms | **~0.5 ms** |

### Insights:
- **Instant Loading:** `rkyv` combined with `mmap` allows the database to be "loaded" in sub-millisecond time regardless of size, as it avoids the entire allocation and copying phase of deserialization.
- **Access Speed:** Once mapped, scanning archived data is **~2.4x faster** than scanning deserialized Bitcode data due to better memory alignment and zero-copy access patterns.
- **Storage Trade-off:** `rkyv` payloads are significantly larger (~4x) than Bitcode because they prioritize memory layout over bit-packing. For an in-memory database where performance is the primary goal, this is a highly acceptable trade-off.
- **FlatBuffers would not improve these numbers:** FlatBuffers has similar (or larger) payload sizes compared to `rkyv` and would require a complete rewrite of the data model to static schemas. There is no performance justification for choosing FlatBuffers over `rkyv` in this architecture.

### Recommendation:
Move toward a full `rkyv` implementation for the primary storage engine to achieve O(1) startup times. For users needing the absolute smallest file size, keep `BitcodeAdapter` as an option. For users needing instant load on massive datasets, implement a dedicated `MmapRkyvStorage` that works directly with archived types.

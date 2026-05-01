use dbobj::core::{ColumnDefinition, DataType, Database, RowData, Schema, Value};
use std::time::Instant;
use string_interner::{DefaultStringInterner, DefaultSymbol, Symbol};

fn main() {
    println!("--- String-Interner Evaluation for DBOBJ ---\n");

    // ------------------------------------------------------------------
    // 1. Baseline: current CompactString approach (via Value::String)
    // ------------------------------------------------------------------
    let db = Database::new("BaselineDB".to_string());
    let schema = Schema {
        columns: vec![ColumnDefinition {
            name: "category".into(),
            data_type: DataType::String,
            nullable: false,
        }],
    };
    db.create_table("products".to_string(), schema);

    let categories = ["Electronics", "Books", "Clothing", "Food", "Toys"];
    let n = 200_000;

    let start = Instant::now();
    for i in 0..n {
        let mut row = RowData::default();
        row.insert(
            "category".into(),
            Value::from(categories[i % categories.len()]),
        );
        db.insert_row("products", row, None).unwrap();
    }
    let insert_time_compact = start.elapsed();
    println!(
        "[Baseline] Inserted {} rows with CompactString: {:?}",
        n, insert_time_compact
    );

    // ------------------------------------------------------------------
    // 2. Evaluate string-interner in isolation
    // ------------------------------------------------------------------
    let mut interner = DefaultStringInterner::default();
    let mut tokens: Vec<u32> = Vec::with_capacity(n);

    let start = Instant::now();
    for i in 0..n {
        let sym: DefaultSymbol = interner.get_or_intern(categories[i % categories.len()]);
        tokens.push(sym.to_usize() as u32);
    }
    let tokenize_time = start.elapsed();
    println!(
        "[Interner] Tokenized {} strings: {:?}",
        n, tokenize_time
    );
    println!(
        "[Interner] Unique strings: {} | Total tokens: {}",
        interner.len(),
        tokens.len()
    );

    // ------------------------------------------------------------------
    // 3. Memory-footprint comparison (rough)
    // ------------------------------------------------------------------
    // CompactString stores up to 24 bytes inline; longer strings heap-alloc.
    // For "Electronics" (11 bytes) it fits inline → ~24 bytes per row.
    // Interner stores one copy per unique string (~5 copies total).
    // Token is a u32 → 4 bytes per row.
    let compact_estimated_bytes = n * std::mem::size_of::<Value>() + n * 24; // very rough
    let interned_estimated_bytes =
        interner.len() * 32 + n * std::mem::size_of::<u32>() + n * std::mem::size_of::<Value>();
    println!(
        "[Memory] CompactString (very rough): ~{} MB",
        compact_estimated_bytes / 1_048_576
    );
    println!(
        "[Memory] Interned (very rough): ~{} MB",
        interned_estimated_bytes / 1_048_576
    );

    // ------------------------------------------------------------------
    // 4. Challenges identified
    // ------------------------------------------------------------------
    println!("\n=== Challenges for integrating string-interner into DBOBJ ===");

    println!("\n1. Global State & Lifetimes");
    println!(
        "   DefaultStringInterner is NOT Send/Sync by default with the \n\
         default backend (HashMapBackend). To share across threads you\n\
         need an Arc<RwLock<StringInterner<...>>> or switch to a \n\
         thread-safe backend. Every insert/query would need access to \n\
         this global pool, complicating the API."
    );

    println!("\n2. rkyv Compatibility");
    println!(
        "   string-interner types do NOT derive rkyv::Archive.\n\
         DBOBJ's storage engine relies on rkyv for fast snapshots.\n\
         Integrating interning would require either:\n\
         - Custom rkyv impls for the interner (very hard).\n\
         - Serializing the interner separately via serde/bitcode (slow).\n\
         - Abandoning rkyv zero-copy for interned fields."
    );

    println!("\n3. API Breakage");
    println!(
        "   Value::String(CompactString) is used everywhere. Changing to\n\
         Value::InternedString(u32) breaks all existing consumers.\n\
         Resolving the u32 back to &str requires &mut or & access to\n\
         the interner at query time, which conflicts with zero-copy\n\
         mmap access."
    );

    println!("\n4. Persistence Complexity");
    println!(
        "   On save you must serialize BOTH the intern pool and the\n\
         tokenized rows. On load you must reconstruct the pool before\n\
         any row can be inspected. This adds significant complexity\n\
         to the Storage adapter layer."
    );

    println!("\n=== Verdict ===");
    println!(
        "string-interner offers large memory savings for datasets with\n\
         high string cardinality, but integrating it into DBOBJ today\n\
         would require either:\n\
         a) Dropping rkyv zero-copy for string fields, or\n\
         b) A massive refactor of Value, Storage, and Query APIs.\n\
         Recommendation: Keep CompactString for now; revisit interning\n\
         if/when a dedicated columnar or OLAP engine is added."
    );
}

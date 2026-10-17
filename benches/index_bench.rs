use ignore::WalkBuilder;
use lsp::indexer::WorkspaceIndex;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tower_lsp::lsp_types::Url;

struct ProfileStats {
    walk_micros: AtomicU64,
    io_micros: AtomicU64,
    parse_symbols_micros: AtomicU64,
    insert_other_micros: AtomicU64,
}

impl ProfileStats {
    fn new() -> Self {
        Self {
            walk_micros: AtomicU64::new(0),
            io_micros: AtomicU64::new(0),
            parse_symbols_micros: AtomicU64::new(0),
            insert_other_micros: AtomicU64::new(0),
        }
    }
}

fn profile_scan(
    root: &Path,
    stats: &Arc<ProfileStats>,
) -> (WorkspaceIndex, usize, std::time::Duration) {
    let total_start = Instant::now();

    // 1. Parallel directory walk
    let walk_start = Instant::now();
    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);

    let (tx, rx) = std::sync::mpsc::channel();
    let walker = WalkBuilder::new(root)
        .standard_filters(true)
        .parents(false)
        .threads(num_threads)
        .build_parallel();

    walker.run(|| {
        let tx = tx.clone();
        Box::new(move |entry| {
            if let Ok(entry) = entry {
                let path = entry.into_path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && (ext == "kt" || ext == "kts")
                {
                    let _ = tx.send(path);
                }
            }
            ignore::WalkState::Continue
        })
    });
    drop(tx);
    let paths: Vec<PathBuf> = rx.into_iter().collect();
    stats
        .walk_micros
        .fetch_add(walk_start.elapsed().as_micros() as u64, Ordering::Relaxed);

    // 2. Parallel I/O + Parsing + Symbol Extraction
    let parsed_files: Vec<(Url, lsp::indexer::FileSymbols)> = paths
        .into_par_iter()
        .filter_map(|path| {
            let io_start = Instant::now();
            let source = std::fs::read_to_string(&path).ok()?;
            stats
                .io_micros
                .fetch_add(io_start.elapsed().as_micros() as u64, Ordering::Relaxed);

            let url = Url::from_file_path(&path).ok()?;

            let sym_start = Instant::now();
            let symbols = lsp::syntax::parser::with_parser(|p| {
                lsp::syntax::symbols::parse_file_symbols(p, &source)
            })?;
            stats
                .parse_symbols_micros
                .fetch_add(sym_start.elapsed().as_micros() as u64, Ordering::Relaxed);

            Some((url, symbols))
        })
        .collect();

    let count = parsed_files.len();

    // 3. Index insertion
    let mut index = WorkspaceIndex::new();
    for (url, file_symbols) in parsed_files {
        let ins_start = Instant::now();
        index.insert_file_symbols(url, file_symbols);
        stats
            .insert_other_micros
            .fetch_add(ins_start.elapsed().as_micros() as u64, Ordering::Relaxed);
    }

    let elapsed = total_start.elapsed();
    (index, count, elapsed)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let target_dir = if args.len() > 1 && !args[1].starts_with('-') {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("benches/kotlinx.coroutines")
    };

    let abs_dir = std::fs::canonicalize(&target_dir).unwrap_or_else(|_| target_dir.clone());
    println!("=== Indexing Benchmark & Profiler ===");
    println!("Target directory: {}", abs_dir.display());

    // Run standard scan_directory
    println!("\n--- Standard scan_directory ---");
    let mut index = WorkspaceIndex::new();
    let start = Instant::now();
    index.scan_directory(&abs_dir);
    let scan_time = start.elapsed();
    let file_count = index.files.len();
    let decl_count: usize = index.files.values().map(|f| f.declarations.len()).sum();
    let name_count = index.name_to_decls.len();
    let container_count = index.container_to_decls.len();

    println!(
        "Scan completed in: {:?}\nFiles indexed: {}\nTotal declarations: {}\nUnique symbol names: {}\nUnique containers: {}",
        scan_time, file_count, decl_count, name_count, container_count
    );

    // Run detailed profiled run
    println!("\n--- Detailed Profiled Run ---");
    let stats = Arc::new(ProfileStats::new());
    let (_idx, count, profiled_time) = profile_scan(&abs_dir, &stats);

    let walk_ms = stats.walk_micros.load(Ordering::Relaxed) as f64 / 1000.0;
    let io_ms = stats.io_micros.load(Ordering::Relaxed) as f64 / 1000.0;
    let parse_sym_ms = stats.parse_symbols_micros.load(Ordering::Relaxed) as f64 / 1000.0;
    let insert_ms = stats.insert_other_micros.load(Ordering::Relaxed) as f64 / 1000.0;

    println!("Total Wall-Clock Time: {:?}", profiled_time);
    println!("Total Files: {}", count);
    println!("Breakdown:");
    println!("  - Directory Walk (Parallel): {:>8.2} ms", walk_ms);
    println!("  - File I/O (Read):           {:>8.2} ms (CPU sum)", io_ms);
    println!(
        "  - Parsing & Symbol Collect:  {:>8.2} ms (CPU sum)",
        parse_sym_ms
    );
    println!("  - Index Insertion:           {:>8.2} ms", insert_ms);
}

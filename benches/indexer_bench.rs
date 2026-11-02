use ahash::AHashMap;
use lsp::features::{find_completions, find_definition, find_hover};
use lsp::indexer::{DeclarationItem, DeclarationMetadata, FileSymbols, ImportItem, WorkspaceIndex};
use smol_str::SmolStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tower_lsp::lsp_types::{Position, Range, SymbolKind, Url};

fn create_declaration_item(
    slot: usize,
    symbol_id: usize,
    class_name: &str,
    doc_sample: &str,
) -> (DeclarationItem, bool) {
    match slot {
        0..=3 => {
            let decl_name = format!("method{slot}");
            (
                DeclarationItem::new(
                    &decl_name,
                    SymbolKind::METHOD,
                    Range::default(),
                    Range::default(),
                    DeclarationMetadata::new(
                        Some(SmolStr::new(doc_sample)),
                        Some(SmolStr::new(format!("fun {decl_name}(x: Int): String"))),
                        Some(SmolStr::new(class_name)),
                        false,
                    ),
                ),
                false,
            )
        }
        4 => (
            DeclarationItem::new(
                class_name,
                SymbolKind::CLASS,
                Range::default(),
                Range::default(),
                DeclarationMetadata::new(
                    Some(SmolStr::new(doc_sample)),
                    Some(SmolStr::new(format!("class {class_name}"))),
                    None,
                    false,
                ),
            ),
            false,
        ),
        5..=8 => {
            let decl_name = format!("topFunc{symbol_id}");
            (
                DeclarationItem::new(
                    &decl_name,
                    SymbolKind::FUNCTION,
                    Range::default(),
                    Range::default(),
                    DeclarationMetadata::new(
                        Some(SmolStr::new(doc_sample)),
                        Some(SmolStr::new(format!("fun {decl_name}(x: Int): String"))),
                        None,
                        false,
                    ),
                ),
                false,
            )
        }
        _ => {
            let decl_name = format!("topFunc{}", symbol_id - 1);
            (
                DeclarationItem::new(
                    &decl_name,
                    SymbolKind::FUNCTION,
                    Range::default(),
                    Range::default(),
                    DeclarationMetadata::new(
                        Some(SmolStr::new(doc_sample)),
                        Some(SmolStr::new(format!("fun {decl_name}(x: Int): String"))),
                        None,
                        false,
                    ),
                ),
                true,
            )
        }
    }
}

fn generate_file_declarations(
    symbol_id: &mut usize,
    total_symbols: usize,
    class_name: &str,
    doc_sample: &str,
) -> (Vec<DeclarationItem>, Vec<DeclarationItem>) {
    let mut src_declarations = Vec::with_capacity(9);
    let mut bc_declarations = Vec::with_capacity(1);

    for slot in 0..10 {
        if *symbol_id >= total_symbols {
            break;
        }
        let (declaration, is_bytecode) =
            create_declaration_item(slot, *symbol_id, class_name, doc_sample);
        if is_bytecode {
            bc_declarations.push(declaration);
        } else {
            src_declarations.push(declaration);
        }
        *symbol_id += 1;
    }
    (src_declarations, bc_declarations)
}

fn generate_dataset(total_symbols: usize) -> Vec<(Url, FileSymbols)> {
    let symbols_per_file = 10;
    let file_count = total_symbols.div_ceil(symbols_per_file);
    let mut files = Vec::with_capacity(file_count * 2);

    let doc_sample = "/**\n * Detailed documentation for symbol.\n * Provides extra context and sample usage.\n * @param x input parameter\n */";

    let mut sym_id = 0;
    for file_index in 0..file_count {
        if sym_id >= total_symbols {
            break;
        }

        let src_uri = Url::parse(&format!("file:///workspace/src/File{file_index}.kt")).unwrap();
        let bc_uri = Url::parse(&format!("file:///workspace/lib/File{file_index}.class")).unwrap();
        let pkg_name = format!("com.example.module{}", file_index % 20);
        let class_name = format!("Class{file_index}");

        let (src_declarations, bc_declarations) =
            generate_file_declarations(&mut sym_id, total_symbols, &class_name, doc_sample);

        let mut class_supertypes = AHashMap::default();
        class_supertypes.insert(class_name, vec!["BaseComponent".to_string()]);

        if !src_declarations.is_empty() {
            files.push((
                src_uri,
                FileSymbols {
                    package_name: Some(pkg_name.clone()),
                    imports: vec![ImportItem {
                        path: "kotlin.collections.*".to_string(),
                        alias: None,
                        is_star: true,
                    }],
                    declarations: src_declarations,
                    class_supertypes,
                    type_aliases: AHashMap::default(),
                },
            ));
        }

        if !bc_declarations.is_empty() {
            files.push((
                bc_uri,
                FileSymbols {
                    package_name: Some(pkg_name),
                    imports: Vec::new(),
                    declarations: bc_declarations,
                    class_supertypes: AHashMap::default(),
                    type_aliases: AHashMap::default(),
                },
            ));
        }
    }

    files
}

fn bench_direct_insertion(dataset: &[(Url, FileSymbols)], total_symbols: usize) -> WorkspaceIndex {
    let mut direct_index = WorkspaceIndex::new();
    let start_time = Instant::now();
    direct_index.insert_batch(dataset.to_vec());
    let insert_duration = start_time.elapsed();
    let throughput = (total_symbols as f64) / insert_duration.as_secs_f64();

    println!(
        "  [Direct Insertion]      {} files / {} symbols in {:.3} ms ({:.0} sym/s)",
        dataset.len(),
        total_symbols,
        insert_duration.as_secs_f64() * 1000.0,
        throughput
    );

    direct_index
}

async fn bench_concurrent_query_latency(dataset: Vec<(Url, FileSymbols)>) {
    let shared_index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let shared_for_writer = Arc::clone(&shared_index);
    let dataset_for_writer = dataset;

    let writer_handle = tokio::spawn(async move {
        const BATCH_SIZE: usize = 500;
        let mut iter = dataset_for_writer.into_iter();
        let start = Instant::now();
        loop {
            let chunk: Vec<_> = iter.by_ref().take(BATCH_SIZE).collect();
            if chunk.is_empty() {
                break;
            }
            {
                let mut shared = shared_for_writer.write().await;
                shared.insert_batch(chunk);
            }
            tokio::task::yield_now().await;
        }
        start.elapsed()
    });

    let mut latencies: Vec<Duration> = Vec::new();
    let target_url = Url::parse("file:///workspace/src/File0.kt").unwrap();

    while !writer_handle.is_finished() {
        let q_start = Instant::now();
        {
            let idx = shared_index.read().await;
            let _ = find_definition(&idx, &target_url, "Class0", Position::default(), "Class0");
        }
        latencies.push(q_start.elapsed());
        tokio::time::sleep(Duration::from_millis(2)).await;
    }

    let writer_time = writer_handle.await.unwrap();

    latencies.sort();
    let p50 = latencies
        .get(latencies.len() / 2)
        .copied()
        .unwrap_or_default();
    let p95 = latencies
        .get(latencies.len() * 95 / 100)
        .copied()
        .unwrap_or_default();
    let p99 = latencies
        .get(latencies.len() * 99 / 100)
        .copied()
        .unwrap_or_default();
    let max_lat = latencies.last().copied().unwrap_or_default();

    println!(
        "  [Concurrent Ingestion]  Writer duration: {:.3} ms",
        writer_time.as_secs_f64() * 1000.0
    );
    println!(
        "  [Reader Interactivity]  Queries: {} | p50: {:.2} µs | p95: {:.2} ms | p99: {:.2} ms | Max freeze: {:.2} ms",
        latencies.len(),
        p50.as_micros() as f64,
        p95.as_secs_f64() * 1000.0,
        p99.as_secs_f64() * 1000.0,
        max_lat.as_secs_f64() * 1000.0
    );
}

fn bench_lsp_queries(direct_index: &WorkspaceIndex, total_symbols: usize) {
    let target_url = Url::parse("file:///workspace/src/File0.kt").unwrap();

    let t_lookup = Instant::now();
    let mut found = 0;
    for i in 0..1000.min(total_symbols) {
        let sym = format!("topFunc{}", i * 10);
        if direct_index.name_to_decls.contains_key(sym.as_str()) {
            found += 1;
        }
    }
    let lookup_duration = t_lookup.elapsed();

    let t_comp = Instant::now();
    let completions = find_completions(
        direct_index,
        &target_url,
        Position {
            line: 0,
            character: 7,
        },
        "Class0.",
    );
    let comp_duration = t_comp.elapsed();

    let t_scope_comp = Instant::now();
    let scope_completions = find_completions(
        direct_index,
        &target_url,
        Position {
            line: 0,
            character: 3,
        },
        "top",
    );
    let scope_comp_duration = t_scope_comp.elapsed();

    let t_hover = Instant::now();
    let hover = find_hover(
        direct_index,
        &target_url,
        "Class0",
        Position {
            line: 0,
            character: 3,
        },
        "Class0",
    );
    let hover_duration = t_hover.elapsed();

    println!(
        "  [LSP Queries]           1k Name Lookups ({} found): {:.3} ms | Member Comp: {:.3} ms ({} items) | Scope Comp: {:.3} ms ({} items) | Hover: {:.3} ms ({})",
        found,
        lookup_duration.as_secs_f64() * 1000.0,
        comp_duration.as_secs_f64() * 1000.0,
        completions.len(),
        scope_comp_duration.as_secs_f64() * 1000.0,
        scope_completions.len(),
        hover_duration.as_secs_f64() * 1000.0,
        if hover.is_some() { "found" } else { "none" }
    );
}

async fn run_magnitude_benchmark(magnitude: usize) {
    println!("\n>>> BENCHMARK MAGNITUDE: {} SYMBOLS <<<", magnitude);
    let dataset = generate_dataset(magnitude);
    let total_symbols: usize = dataset.iter().map(|(_, fs)| fs.declarations.len()).sum();

    let direct_index = bench_direct_insertion(&dataset, total_symbols);
    bench_concurrent_query_latency(dataset).await;
    bench_lsp_queries(&direct_index, total_symbols);
}

#[tokio::main]
async fn main() {
    println!("============================================================");
    println!("           KOTLIN-LSP INDEXER BENCHMARK SUITE               ");
    println!("============================================================");

    let magnitudes = [10_000, 100_000, 300_000, 500_000, 1_000_000];

    for &magnitude in &magnitudes {
        run_magnitude_benchmark(magnitude).await;
    }
}

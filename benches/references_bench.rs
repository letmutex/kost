use ahash::AHashSet;
use lsp::WorkspaceIndex;
use lsp::features::find_references;
use lsp::indexer::is_source_url;
use lsp::syntax::symbol_item::DeclarationItem;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tower_lsp::lsp_types::{SymbolKind, Url};

struct BenchResult {
    symbol_name: String,
    kind_label: &'static str,
    file_relative_path: String,
    line: u32,
    col: u32,
    elapsed_seconds: f64,
    reference_count: usize,
    files_with_refs_count: usize,
}

fn symbol_kind_to_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::CLASS => "Class",
        SymbolKind::INTERFACE => "Interface",
        SymbolKind::FUNCTION => "Function",
        SymbolKind::METHOD => "Method",
        SymbolKind::OBJECT => "Object",
        SymbolKind::PROPERTY => "Property",
        SymbolKind::VARIABLE => "Variable",
        _ => "Symbol",
    }
}

fn resolve_workspace_dir(cli_path_arg: Option<&str>) -> PathBuf {
    if let Some(arg) = cli_path_arg {
        let requested_path = PathBuf::from(arg);
        if requested_path.exists() {
            return requested_path;
        }
        if arg == "benches/coroutines" && Path::new("benches/kotlinx.coroutines").exists() {
            println!("Note: 'benches/coroutines' redirected to 'benches/kotlinx.coroutines'");
            return PathBuf::from("benches/kotlinx.coroutines");
        }
        return requested_path;
    }

    if Path::new("benches/coroutines").exists() {
        PathBuf::from("benches/coroutines")
    } else {
        PathBuf::from("benches/kotlinx.coroutines")
    }
}

fn is_container_private(index: &WorkspaceIndex, url: &Url, container: &str) -> bool {
    let Some(file_syms) = index.files.get(url) else {
        return false;
    };
    container.split('.').any(|segment| {
        file_syms.declarations.iter().any(|d| {
            d.name == segment
                && d.is_private
                && matches!(
                    d.kind,
                    SymbolKind::CLASS | SymbolKind::OBJECT | SymbolKind::INTERFACE
                )
        })
    })
}

fn find_best_declaration(
    index: &WorkspaceIndex,
    target_name: &str,
    preferred_kind: Option<SymbolKind>,
) -> Option<(Url, DeclarationItem)> {
    let clean_name = target_name.trim().trim_end_matches("()").trim();
    let (target_container, member_name) = match clean_name.split_once('.') {
        Some((container, member)) => (Some(container), member),
        None => (None, clean_name),
    };

    let decls = index.name_to_decls.get(member_name)?;
    let mut matches: Vec<(&Url, &DeclarationItem)> = Vec::new();

    for (url, decl) in decls {
        if !is_source_url(url) {
            continue;
        }

        if let Some(container) = target_container {
            let matches_container =
                decl.container_name
                    .as_deref()
                    .is_some_and(|declared_container| {
                        declared_container.eq_ignore_ascii_case(container)
                    });
            if !matches_container {
                continue;
            }
        } else if decl.is_private
            || decl
                .container_name
                .as_deref()
                .is_some_and(|container| is_container_private(index, url, container))
        {
            continue;
        }

        let matches_preferred_kind = match preferred_kind {
            Some(kind) => {
                decl.kind == kind
                    || (matches!(kind, SymbolKind::FUNCTION | SymbolKind::METHOD)
                        && matches!(decl.kind, SymbolKind::FUNCTION | SymbolKind::METHOD))
            }
            None => true,
        };

        if matches_preferred_kind {
            matches.push((url.as_ref(), decl.as_ref()));
        }
    }

    matches.sort_by(|(url_a, decl_a), (url_b, decl_b)| {
        url_a
            .as_str()
            .cmp(url_b.as_str())
            .then_with(|| decl_a.range.start.line.cmp(&decl_b.range.start.line))
            .then_with(|| {
                decl_a
                    .range
                    .start
                    .character
                    .cmp(&decl_b.range.start.character)
            })
    });

    matches
        .first()
        .map(|(url, decl)| ((**url).clone(), (**decl).clone()))
}

fn benchmark_target_symbol(
    index: &WorkspaceIndex,
    workspace_root: &Path,
    target_name: &str,
    preferred_kind: Option<SymbolKind>,
) -> Option<BenchResult> {
    let resolve_start = Instant::now();
    let (decl_url, decl_item) = find_best_declaration(index, target_name, preferred_kind)?;
    let source_text = index.get_file_source(&decl_url)?;
    let resolve_elapsed = resolve_start.elapsed();

    let start_position = decl_item.selection_range.start;
    let kind_label = symbol_kind_to_label(decl_item.kind);

    let relative_file_path = decl_url
        .to_file_path()
        .ok()
        .and_then(|path| {
            path.strip_prefix(workspace_root)
                .ok()
                .map(|relative_path| relative_path.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| decl_url.to_string());

    let bench_start = Instant::now();
    let locations = find_references(
        index,
        &decl_url,
        &decl_item.name,
        start_position,
        &source_text,
        false,
    );
    let search_elapsed = bench_start.elapsed();
    let total_elapsed = resolve_elapsed + search_elapsed;

    let unique_file_uris: AHashSet<&Url> = locations.iter().map(|loc| &loc.uri).collect();

    Some(BenchResult {
        symbol_name: target_name.to_string(),
        kind_label,
        file_relative_path: relative_file_path,
        line: start_position.line + 1,
        col: start_position.character + 1,
        elapsed_seconds: total_elapsed.as_secs_f64(),
        reference_count: locations.len(),
        files_with_refs_count: unique_file_uris.len(),
    })
}

fn parse_cli_options() -> (PathBuf, Vec<String>) {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut custom_path = None;
    let mut custom_symbols = Vec::new();

    for arg in args {
        if arg.starts_with('-') {
            continue;
        }
        let candidate_path = Path::new(&arg);
        if custom_path.is_none()
            && (candidate_path.exists() || arg.contains('/') || arg.contains('\\'))
        {
            custom_path = Some(arg);
        } else {
            custom_symbols.push(arg);
        }
    }

    let workspace_dir = resolve_workspace_dir(custom_path.as_deref());
    (workspace_dir, custom_symbols)
}

fn print_results_summary_table(results: &[BenchResult]) {
    println!("\n{:=<95}", "");
    println!(
        "{:<24} {:<12} {:>14} {:>12} {:>12}   {:<15}",
        "Symbol", "Kind", "Time (sec)", "References", "Files", "Declared At"
    );
    println!("{:-<95}", "");

    for result in results {
        let location_tag = format!(
            "{}:{}:{}",
            result.file_relative_path, result.line, result.col
        );
        println!(
            "{:<24} {:<12} {:>13.3}s {:>12} {:>12}   {}",
            result.symbol_name,
            result.kind_label,
            result.elapsed_seconds,
            result.reference_count,
            result.files_with_refs_count,
            location_tag
        );
    }
    println!("{:=<95}\n", "");
}

fn main() {
    let (workspace_dir, requested_symbols) = parse_cli_options();
    let abs_workspace_dir =
        std::fs::canonicalize(&workspace_dir).unwrap_or_else(|_| workspace_dir.clone());

    println!("===============================================================");
    println!("        FIND REFERENCES BENCHMARK: KOTLINX.COROUTINES          ");
    println!("===============================================================");
    println!("Target directory: {}", abs_workspace_dir.display());

    if !abs_workspace_dir.exists() {
        eprintln!(
            "Error: directory '{}' does not exist.\nEnsure the submodule is cloned or provide a valid path.",
            abs_workspace_dir.display()
        );
        std::process::exit(1);
    }

    println!("\nIndexing workspace directory...");
    let mut index = WorkspaceIndex::new();
    let index_start = Instant::now();
    index.scan_directory(&abs_workspace_dir);
    let index_time = index_start.elapsed();

    let total_files = index.files.len();
    let total_decls: usize = index.files.values().map(|f| f.declarations.len()).sum();

    println!(
        "Indexed {} files with {} declarations in {:.2} ms",
        total_files,
        total_decls,
        index_time.as_secs_f64() * 1000.0
    );

    let default_queries = [
        ("Job", Some(SymbolKind::INTERFACE)),
        ("CoroutineScope", Some(SymbolKind::INTERFACE)),
        ("CoroutineDispatcher", Some(SymbolKind::CLASS)),
        ("NonCancellable", Some(SymbolKind::OBJECT)),
        ("launch", Some(SymbolKind::FUNCTION)),
        ("job.cancel", Some(SymbolKind::FUNCTION)),
        ("Job.isActive", Some(SymbolKind::PROPERTY)),
        ("Empty.toString", Some(SymbolKind::FUNCTION)),
        ("Symbol.toString", Some(SymbolKind::FUNCTION)),
        ("NonCancellable.toString", Some(SymbolKind::FUNCTION)),
    ];

    let mut results = Vec::new();

    if requested_symbols.is_empty() {
        println!("\nRunning references benchmark on standard target symbols...");
        for &(query_name, preferred_kind) in &default_queries {
            print!("Benchmarking '{query_name}' ... ");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            if let Some(bench_result) =
                benchmark_target_symbol(&index, &abs_workspace_dir, query_name, preferred_kind)
            {
                println!(
                    "done in {:.3}s ({} refs in {} files)",
                    bench_result.elapsed_seconds,
                    bench_result.reference_count,
                    bench_result.files_with_refs_count
                );
                results.push(bench_result);
            } else {
                println!("target declaration not found!");
            }
        }
    } else {
        println!("\nRunning references benchmark on requested symbol(s)...");
        for symbol_name in &requested_symbols {
            print!("Benchmarking '{}' ... ", symbol_name);
            let _ = std::io::Write::flush(&mut std::io::stdout());
            if let Some(bench_result) =
                benchmark_target_symbol(&index, &abs_workspace_dir, symbol_name, None)
            {
                println!(
                    "done in {:.3}s ({} refs in {} files)",
                    bench_result.elapsed_seconds,
                    bench_result.reference_count,
                    bench_result.files_with_refs_count
                );
                results.push(bench_result);
            } else {
                println!("target declaration not found!");
            }
        }
    }

    if !results.is_empty() {
        print_results_summary_table(&results);
    }
}

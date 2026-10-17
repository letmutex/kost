use crossbeam_channel::bounded;
use std::path::{Path, PathBuf};
use url::Url;

use super::WorkspaceIndex;
use super::types::FileSymbols;
use super::walker::spawn_directory_walker;
use crate::parser::KotlinParser;
use crate::symbols::parse_file_symbols;

pub fn scan_directory(index: &mut WorkspaceIndex, root: &Path) {
    let start = std::time::Instant::now();
    let abs_root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    crate::logger::log!(
        "indexing",
        "Scanning workspace directory '{}'...",
        abs_root.display()
    );

    let num_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);

    let (path_tx, path_rx) = bounded::<PathBuf>(1024);
    let (result_tx, result_rx) = bounded::<(Url, FileSymbols)>(1024);
    let walker_handle = spawn_directory_walker(abs_root, num_threads.min(8), path_tx);

    let mut workers = Vec::with_capacity(num_threads);
    for _ in 0..num_threads {
        let rx = path_rx.clone();
        let tx = result_tx.clone();
        workers.push(std::thread::spawn(move || {
            let mut parser = KotlinParser::new();
            while let Ok(path) = rx.recv() {
                let Ok(source) = std::fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(url) = Url::from_file_path(&path) else {
                    continue;
                };
                if let Some(symbols) = parse_file_symbols(&mut parser, &source) {
                    let _ = tx.send((url, symbols));
                }
            }
        }));
    }
    drop(result_tx);
    drop(path_rx);

    let mut count = 0;
    while let Ok((url, symbols)) = result_rx.recv() {
        index.insert_file_symbols(url, symbols);
        count += 1;
    }

    let _ = walker_handle.join();
    for w in workers {
        let _ = w.join();
    }

    crate::logger::log!(
        "indexing",
        "scan_directory: indexed {} files in {:?}",
        count,
        start.elapsed()
    );
}

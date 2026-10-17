use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use smol_str::SmolStr;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use tower_lsp::lsp_types::Url;
use zip::ZipArchive;

use crate::gradle::class_file::parse_class_file_symbols;
use crate::indexer::FileSymbols;
use crate::parser::with_parser;
use crate::symbols::parse_file_symbols;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveProcessingMetric {
    pub path: PathBuf,
    pub duration: std::time::Duration,
    pub symbol_count: usize,
    pub is_aar: bool,
}

enum EntryPayload {
    Source(String),
    Bytecode(Vec<u8>),
}

struct PendingEntry {
    name: String,
    url_prefix: SmolStr,
    payload: EntryPayload,
}

/// In-memory archive scanner for JAR, AAR, and Sources JAR.
pub struct ArchiveIndexer;

impl ArchiveIndexer {
    /// Index a single archive (JAR, AAR, or Sources JAR) directly in-memory.
    pub fn index_archive(archive_path: &Path, prefer_sources: bool) -> Vec<(Url, FileSymbols)> {
        Self::index_archive_with_metric(archive_path, prefer_sources).0
    }

    /// Index a single archive with elapsed duration and metric tracking.
    pub fn index_archive_with_metric(
        archive_path: &Path,
        _prefer_sources: bool,
    ) -> (Vec<(Url, FileSymbols)>, ArchiveProcessingMetric) {
        let start = std::time::Instant::now();
        let is_aar = archive_path.extension().is_some_and(|e| e == "aar");

        let symbols = (|| {
            let Ok(archive_bytes) = std::fs::read(archive_path) else {
                return Vec::new();
            };
            let cursor = Cursor::new(archive_bytes);
            let Ok(mut zip) = ZipArchive::new(cursor) else {
                return Vec::new();
            };

            let archive_str = archive_path.to_string_lossy().replace('\\', "/");
            let pending_entries = if is_aar {
                Self::extract_aar_entries(&mut zip, &archive_str)
            } else {
                let url_prefix = compute_url_prefix(&archive_str);
                Self::extract_zip_entries(&mut zip, &url_prefix)
            };

            drop(zip);

            pending_entries
                .into_par_iter()
                .filter_map(|entry| {
                    let synthetic_url = crate::concat_str!(entry.url_prefix, entry.name);
                    let url = Url::parse(&synthetic_url).ok()?;
                    let symbols = match entry.payload {
                        EntryPayload::Source(content) => {
                            with_parser(|p| parse_file_symbols(p, &content))?
                        }
                        EntryPayload::Bytecode(bytes) => parse_class_file_symbols(&bytes)?,
                    };
                    Some((url, symbols))
                })
                .collect()
        })();

        let metric = ArchiveProcessingMetric {
            path: archive_path.to_path_buf(),
            duration: start.elapsed(),
            symbol_count: symbols.len(),
            is_aar,
        };

        (symbols, metric)
    }

    /// Parallel batch indexing of multiple archives using Rayon.
    pub fn index_archives_parallel(
        archives: &[&Path],
        prefer_sources: bool,
    ) -> (Vec<(Url, FileSymbols)>, Vec<ArchiveProcessingMetric>) {
        let results: Vec<_> = archives
            .par_iter()
            .map(|path| Self::index_archive_with_metric(path, prefer_sources))
            .collect();
        let mut all_symbols = Vec::new();
        let mut all_metrics = Vec::with_capacity(results.len());
        for (syms, metric) in results {
            all_symbols.extend(syms);
            all_metrics.push(metric);
        }
        (all_symbols, all_metrics)
    }

    fn extract_aar_entries<R: Read + std::io::Seek>(
        zip: &mut ZipArchive<R>,
        archive_str: &str,
    ) -> Vec<PendingEntry> {
        let mut pending = Vec::new();
        for i in 0..zip.len() {
            if let Ok(mut entry) = zip.by_index(i) {
                let Ok(entry_name) = entry.name() else {
                    continue;
                };
                let is_match = entry_name == "classes.jar"
                    || (entry_name.starts_with("libs/") && entry_name.ends_with(".jar"));
                if is_match {
                    let name = entry_name.to_string();
                    let mut inner_bytes = Vec::with_capacity(entry.size() as usize);
                    if entry.read_to_end(&mut inner_bytes).is_ok() {
                        let inner_cursor = Cursor::new(inner_bytes);
                        if let Ok(mut inner_zip) = ZipArchive::new(inner_cursor) {
                            let inner_archive_base = crate::concat_str!(archive_str, "!/", name);
                            let url_prefix = compute_url_prefix(&inner_archive_base);
                            pending.extend(Self::extract_zip_entries(&mut inner_zip, &url_prefix));
                        }
                    }
                }
            }
        }
        pending
    }

    fn extract_zip_entries<R: Read + std::io::Seek>(
        zip: &mut ZipArchive<R>,
        url_prefix: &SmolStr,
    ) -> Vec<PendingEntry> {
        let mut pending = Vec::new();
        for i in 0..zip.len() {
            let Ok(mut entry) = zip.by_index(i) else {
                continue;
            };
            if entry.is_dir() {
                continue;
            }
            let Ok(entry_name) = entry.name() else {
                continue;
            };
            let is_source = entry_name.ends_with(".kt") || entry_name.ends_with(".kts");
            let is_class = entry_name.ends_with(".class")
                && !entry_name.contains('$')
                && !entry_name.ends_with("module-info.class");

            if is_source {
                let name = entry_name.to_string();
                let mut content = String::with_capacity(entry.size() as usize);
                if entry.read_to_string(&mut content).is_ok() {
                    pending.push(PendingEntry {
                        name,
                        url_prefix: url_prefix.clone(),
                        payload: EntryPayload::Source(content),
                    });
                }
            } else if is_class {
                let name = entry_name.to_string();
                let mut bytes = Vec::with_capacity(entry.size() as usize);
                if entry.read_to_end(&mut bytes).is_ok() {
                    pending.push(PendingEntry {
                        name,
                        url_prefix: url_prefix.clone(),
                        payload: EntryPayload::Bytecode(bytes),
                    });
                }
            }
        }
        pending
    }
}

fn compute_url_prefix(archive_uri_base: &str) -> SmolStr {
    let clean_base = archive_uri_base.trim_start_matches('/');
    if clean_base.contains("://") {
        crate::concat_smol_str!("jar:", clean_base, "!/")
    } else {
        crate::concat_smol_str!("jar:file:///", clean_base, "!/")
    }
}

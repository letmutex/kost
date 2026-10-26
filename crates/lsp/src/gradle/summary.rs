use std::path::PathBuf;
use std::time::Duration;

use crate::config::IndexingLogLevel;
use crate::gradle::archive::ArchiveProcessingMetric;
use crate::gradle::cache_resolver::ResolvedArtifact;
use common::concat_str;

/// Summary of an indexing run across Gradle build files, dependencies, and archives.
#[derive(Debug, Clone, Default)]
pub struct IndexingSummary {
    pub catalogs_found: usize,
    pub build_scripts_found: usize,
    pub total_dependencies_declared: usize,
    pub unique_dependencies_indexed: usize,
    pub graph_dependencies_resolved: usize,
    pub direct_dependencies_resolved: usize,
    pub transitive_dependencies_indexed: usize,
    pub transitive_dependencies_resolved: usize,
    pub descriptors_parsed: usize,
    pub android_platform: Option<String>,
    pub android_sdk_jar: Option<PathBuf>,
    pub kotlin_version: Option<String>,
    pub total_symbols_indexed: usize,
    pub resolved_artifacts: Vec<ResolvedArtifact>,
    pub archive_metrics: Vec<ArchiveProcessingMetric>,
    pub duration: Duration,
}

impl IndexingSummary {
    /// Total number of archive files (.jar, .aar, etc.) that were indexed.
    pub fn total_archives_indexed(&self) -> usize {
        self.archive_metrics.len()
    }

    /// Number of AAR archive files indexed.
    pub fn aar_archives_indexed(&self) -> usize {
        self.archive_metrics.iter().filter(|m| m.is_aar).count()
    }

    /// Number of JAR archive files indexed.
    pub fn jar_archives_indexed(&self) -> usize {
        self.total_archives_indexed()
            .saturating_sub(self.aar_archives_indexed())
    }

    pub fn print_resolved_artifacts(&self, level: IndexingLogLevel) {
        self.print_summary(level);
    }

    fn format_archive_summary(&self) -> String {
        let total = self.total_archives_indexed();
        if total > 0 {
            let aar = self.aar_archives_indexed();
            let jar = self.jar_archives_indexed();
            concat_str!(
                "Indexed ",
                &total.to_string(),
                " archives (",
                &jar.to_string(),
                " JAR, ",
                &aar.to_string(),
                " AAR; ",
                &self.total_symbols_indexed.to_string(),
                " symbols)"
            )
        } else {
            concat_str!(
                "Indexed 0 archives (",
                &self.total_symbols_indexed.to_string(),
                " symbols)"
            )
        }
    }

    fn format_dependency_summary(&self) -> String {
        let direct = if self.direct_dependencies_resolved > 0 {
            self.direct_dependencies_resolved
        } else {
            self.unique_dependencies_indexed
                .saturating_sub(self.transitive_dependencies_resolved)
        };
        let transitive = if self.transitive_dependencies_indexed > 0 {
            self.transitive_dependencies_indexed
        } else {
            self.unique_dependencies_indexed.saturating_sub(direct)
        };

        if self.graph_dependencies_resolved > 0 {
            let missing = self
                .graph_dependencies_resolved
                .saturating_sub(self.unique_dependencies_indexed);
            let missing_part = if missing > 0 {
                concat_str!(", ", &missing.to_string(), " missing from cache")
            } else {
                String::new()
            };
            concat_str!(
                " across ",
                &self.unique_dependencies_indexed.to_string(),
                " resolved libraries (",
                &direct.to_string(),
                " direct, ",
                &transitive.to_string(),
                " transitive) from ",
                &self.graph_dependencies_resolved.to_string(),
                " graph dependencies (parsed ",
                &self.descriptors_parsed.to_string(),
                " descriptors",
                &missing_part,
                ")"
            )
        } else if self.descriptors_parsed > 0 || self.transitive_dependencies_resolved > 0 {
            concat_str!(
                " across ",
                &self.unique_dependencies_indexed.to_string(),
                " libraries (",
                &direct.to_string(),
                " direct, ",
                &self.transitive_dependencies_resolved.to_string(),
                " transitive, parsed ",
                &self.descriptors_parsed.to_string(),
                " descriptors)"
            )
        } else if self.unique_dependencies_indexed > 0 {
            concat_str!(
                " across ",
                &self.unique_dependencies_indexed.to_string(),
                " resolved libraries"
            )
        } else {
            String::new()
        }
    }

    /// Formats the single-line summary message explicitly detailing processed archives,
    /// resolved library dependencies, graph dependencies, and environment info.
    pub fn summary_display_line(&self) -> String {
        let archive_part = self.format_archive_summary();
        let dep_part = self.format_dependency_summary();
        let sdk_name = self.android_platform.as_deref().unwrap_or("none");
        let kotlin_ver = self.kotlin_version.as_deref().unwrap_or("none");
        let duration_str = format!("{:?}", self.duration);

        concat_str!(
            &archive_part,
            &dep_part,
            ", Android SDK: ",
            sdk_name,
            ", Kotlin: ",
            kotlin_ver,
            " in ",
            &duration_str
        )
    }

    pub fn print_summary(&self, level: IndexingLogLevel) {
        match level {
            IndexingLogLevel::None => {}
            IndexingLogLevel::Default => {
                crate::logger::log!("indexing", "{}", self.summary_display_line());
            }
            IndexingLogLevel::Verbose => {
                for metric in &self.archive_metrics {
                    let is_sources = metric
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.ends_with("-sources.jar"));
                    let kind = if metric.is_aar {
                        "aar"
                    } else if is_sources {
                        "sources jar"
                    } else {
                        "jar"
                    };
                    let file_name = metric
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_else(|| metric.path.to_str().unwrap_or(""));
                    crate::logger::log!(
                        "indexing",
                        "Processed {kind}: {} ({} symbols) in {:?}",
                        file_name,
                        metric.symbol_count,
                        metric.duration
                    );
                }
                crate::logger::log!("indexing", "{}", self.summary_display_line());
            }
        }
    }
}

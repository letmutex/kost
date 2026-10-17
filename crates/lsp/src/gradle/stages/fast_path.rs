use std::path::Path;
use std::time::Instant;
use tower_lsp::lsp_types::Url;

use super::{log_resolved_artifact, prepare_archive_paths};
use crate::gradle::android_sdk::{AndroidSdkPlatform, AndroidSdkResolver};
use crate::gradle::archive::{ArchiveIndexer, ArchiveProcessingMetric};
use crate::gradle::cache_resolver::{GradleCacheResolver, ResolvedArtifact};
use crate::gradle::config::GradleConfig;
use crate::gradle::kotlinc::KotlincResolver;
use crate::indexer::FileSymbols;

pub struct FastPathResult {
    pub android_platform: Option<AndroidSdkPlatform>,
    pub stdlib_artifact: Option<ResolvedArtifact>,
    pub symbols: Vec<(Url, FileSymbols)>,
    pub metrics: Vec<ArchiveProcessingMetric>,
    pub kotlin_version: String,
}

fn resolve_android_platform(
    workspace_root: &Path,
    config: &GradleConfig,
    pref_sdk: Option<u32>,
) -> Option<AndroidSdkPlatform> {
    let platform = pref_sdk.and_then(|sdk| {
        AndroidSdkResolver::find_sdk_root(Some(workspace_root), config)
            .and_then(|root| AndroidSdkResolver::resolve_platform(&root, Some(sdk)))
    });
    if let Some(ref p) = platform
        && config.indexing_logging.is_verbose()
    {
        crate::logger::log!(
            "indexing",
            "Resolved Android SDK: {} -> {}",
            p.platform_name,
            p.android_jar.display()
        );
    }
    platform
}

fn resolve_stdlib_artifact(
    config: &GradleConfig,
    kotlin_version: Option<&str>,
    has_gradle_files: bool,
) -> Option<ResolvedArtifact> {
    let stdlib = if has_gradle_files {
        GradleCacheResolver::find_kotlin_stdlib(kotlin_version, config)
            .or_else(|| KotlincResolver::find_kotlinc_stdlib(config))
    } else {
        KotlincResolver::find_kotlinc_stdlib(config)
    };
    if let Some(ref s) = stdlib
        && config.indexing_logging.is_verbose()
    {
        log_resolved_artifact(s, config.prefer_sources);
    }
    stdlib
}

pub fn run_fast_path_stage(
    workspace_root: &Path,
    config: &GradleConfig,
    kotlin_version: Option<&str>,
    pref_sdk: Option<u32>,
    has_gradle_files: bool,
) -> FastPathResult {
    let start = Instant::now();
    if config.indexing_logging.is_enabled() {
        crate::logger::log!(
            "indexing",
            "Stage 1/3: Fast path indexing Kotlin stdlib & Android SDK..."
        );
    }

    let android_platform = resolve_android_platform(workspace_root, config, pref_sdk);
    let stdlib_artifact = resolve_stdlib_artifact(config, kotlin_version, has_gradle_files);

    let mut artifacts = Vec::new();
    if let Some(ref stdlib) = stdlib_artifact {
        artifacts.push(stdlib.clone());
    }

    let archives =
        prepare_archive_paths(&artifacts, android_platform.as_ref(), config.prefer_sources);
    let (symbols, metrics) =
        ArchiveIndexer::index_archives_parallel(&archives, config.prefer_sources);
    let total_syms: usize = metrics.iter().map(|m| m.symbol_count).sum();

    let kotlin_version =
        resolve_stdlib_version_display(stdlib_artifact.as_ref(), config.prefer_sources);

    if config.indexing_logging.is_enabled() {
        let sdk_name = android_platform
            .as_ref()
            .map(|p| p.platform_name.as_str())
            .unwrap_or("none");
        crate::logger::log!(
            "indexing",
            "Stage 1/3 done in {:?}: {} symbols from {} archives (Stdlib: {}, Android SDK: {})",
            start.elapsed(),
            total_syms,
            archives.len(),
            kotlin_version,
            sdk_name
        );
    }

    FastPathResult {
        android_platform,
        stdlib_artifact,
        symbols,
        metrics,
        kotlin_version,
    }
}

pub fn resolve_stdlib_version_display(
    artifact: Option<&ResolvedArtifact>,
    prefer_sources: bool,
) -> String {
    let Some(stdlib) = artifact else {
        return "none".to_string();
    };
    if let Some(ref v) = stdlib.coordinate.version {
        return v.raw().to_string();
    }
    let file_name = stdlib
        .target_path(prefer_sources)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    extract_version_from_filename(file_name).unwrap_or_else(|| "kotlinc runtime".to_string())
}

pub fn extract_version_from_filename(file_name: &str) -> Option<String> {
    static VER_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = VER_RE.get_or_init(|| {
        regex::Regex::new(r#"kotlin-stdlib-(?:jvm-)?(\d+\.\d+(?:\.\d+)?(?:-[a-zA-Z0-9.]+)?)"#)
            .unwrap()
    });
    let cap = re.captures(file_name)?;
    let v = cap.get(1)?.as_str();
    let clean = v.trim_end_matches("-sources").trim_end_matches(".jar");
    Some(clean.to_string())
}

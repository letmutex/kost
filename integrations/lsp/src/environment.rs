use ahash::AHashSet;
use common::concat_str;
use lsp::WorkspaceIndex;
use lsp::gradle::archive::ArchiveIndexer;
use lsp::gradle::cache_resolver::GradleCacheResolver;
use lsp::gradle::config::GradleConfig;
use lsp::gradle::dependency::deduplicate_newest;
use lsp::gradle::discovery::discover_workspace_pipeline_context;
use lsp::gradle::graph::DependencyGraph;
use lsp::gradle::kotlinc::KotlincResolver;
use resolver::ResolvedArtifact;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tower_lsp::lsp_types::Url;

use crate::config::{ProjectConfig, resolve_project_root_path, strip_unc_prefix};

pub struct WorkspaceEnvironment {
    pub project_name: String,
    pub root_path: PathBuf,
    pub index: WorkspaceIndex,
}

impl WorkspaceEnvironment {
    pub fn load_project(
        config_file_path: &Path,
        project_config: &ProjectConfig,
    ) -> Result<Self, String> {
        let raw_root_path = resolve_project_root_path(config_file_path, &project_config.path);
        let root_path = strip_unc_prefix(&raw_root_path);
        if !root_path.exists() {
            let error_msg = concat_str!(
                "Project root directory does not exist: ",
                &root_path.to_string_lossy(),
                " (configured as '",
                &project_config.path,
                "')"
            );
            return Err(error_msg);
        }

        let mut index = WorkspaceIndex::new();
        let scan_start = Instant::now();
        index.scan_directory(&root_path);
        let scan_duration = scan_start.elapsed();

        let scanned_file_count = index.files.len();
        let declaration_count: usize = index
            .files
            .values()
            .map(|file_symbols| file_symbols.declarations.len())
            .sum();

        let duration_ms = scan_duration.as_secs_f64() * 1000.0;
        println!(
            "Indexed {} files ({} declarations) in {:.2} ms for '{}'",
            scanned_file_count, declaration_count, duration_ms, project_config.name
        );

        if project_config.indexing.index_dependencies {
            index_project_dependencies(&mut index, &root_path);
        }

        Ok(Self {
            project_name: project_config.name.clone(),
            root_path,
            index,
        })
    }

    pub fn resolve_file_url(&self, relative_path: &str) -> Result<Url, String> {
        let absolute_file_path = self.root_path.join(relative_path);
        Url::from_file_path(&absolute_file_path).map_err(|_| {
            concat_str!(
                "Failed to convert file path to Url: ",
                &absolute_file_path.to_string_lossy()
            )
        })
    }

    pub fn get_file_source(&self, url: &Url) -> Option<String> {
        self.index.get_file_source(url)
    }
}

fn resolve_dependency_artifacts(
    project_root: &Path,
    gradle_config: &GradleConfig,
) -> Vec<ResolvedArtifact> {
    let discovery = discover_workspace_pipeline_context(project_root);
    let deduped_deps = deduplicate_newest(discovery.declared_dependencies);

    let gradle_home = gradle_config.resolve_gradle_user_home();
    let graph_result =
        DependencyGraph::resolve_transitive_graph(&deduped_deps, gradle_home.as_deref());

    let mut resolved_artifacts =
        GradleCacheResolver::resolve_all(&graph_result.resolved_dependencies, gradle_config);

    let stdlib_artifact = KotlincResolver::find_kotlinc_stdlib(gradle_config)
        .or_else(|| GradleCacheResolver::find_kotlin_stdlib(None, gradle_config));
    if let Some(stdlib) = stdlib_artifact
        && !resolved_artifacts
            .iter()
            .any(|artifact| artifact.binary_path == stdlib.binary_path)
    {
        resolved_artifacts.push(stdlib);
    }

    resolved_artifacts
}

fn prepare_dependency_archives(resolved_artifacts: &[ResolvedArtifact]) -> Vec<&Path> {
    let mut archives = Vec::new();
    let mut seen = AHashSet::default();
    for artifact in resolved_artifacts {
        let path = artifact.target_path(true);
        if seen.insert(path) {
            archives.push(path);
        }
    }
    archives
}

fn index_project_dependencies(index: &mut WorkspaceIndex, project_root: &Path) {
    let gradle_config = GradleConfig::new();
    let start_time = Instant::now();

    let resolved_artifacts = resolve_dependency_artifacts(project_root, &gradle_config);
    if resolved_artifacts.is_empty() {
        return;
    }

    let archives = prepare_dependency_archives(&resolved_artifacts);
    let (symbols, _) = ArchiveIndexer::index_archives_parallel(&archives, true);
    let symbol_count = symbols.len();
    for (url, file_symbols) in symbols {
        index.insert_file_symbols(url, file_symbols);
    }

    let elapsed_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    println!(
        "Indexed {} dependency archive(s) ({} symbols) in {:.2} ms",
        archives.len(),
        symbol_count,
        elapsed_ms
    );
}

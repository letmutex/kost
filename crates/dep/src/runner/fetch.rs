use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;
use crate::downloader::Downloader;
use crate::graph::{GraphNode, SolvedGraph};
use crate::repo::RepositoryRegistry;
use cli::{DepFetchArgs, render_fetch_help};

pub async fn run_fetch(args: DepFetchArgs) -> Result<(), String> {
    if args.help {
        print!("{}", render_fetch_help());
        return Ok(());
    }

    validate_gradle_home(args.gradle_home.as_deref())?;

    let mut ctx = super::prepare_context(
        args.project_dir.as_deref(),
        args.gradle_home.as_deref(),
        &args.repositories,
        &args.coordinates,
    )?;
    let downloader = Downloader::default();

    super::resolve_unspecified_coordinates(
        &mut ctx.direct_deps,
        &ctx.registry,
        &ctx.cache,
        &downloader,
        false,
        true,
    )
    .await?;

    let mut progress = if args.transitive {
        Some(crate::progress::ResolveProgress::default_for_stderr())
    } else {
        None
    };

    let graph = build_fetch_graph_with_progress(
        &ctx.direct_deps,
        &ctx.registry,
        &ctx.cache,
        &downloader,
        args.transitive,
        false,
        progress.as_mut(),
    )
    .await;

    println!(
        "Solved {} dependencies ({} direct, {} transitive).",
        graph.nodes.len(),
        graph.direct_count,
        graph.transitive_count
    );

    let (binaries, sources, failed_binaries) = download_fetch_artifacts(
        &graph,
        &ctx.registry,
        &ctx.cache,
        &downloader,
        args.sources,
        false,
    )
    .await;

    if failed_binaries > 0 {
        return Err(common::concat_str!(
            "Failed to download ",
            &failed_binaries.to_string(),
            " binary artifact(s)."
        ));
    }

    print_fetch_summary(&ctx.cache, binaries, sources);
    Ok(())
}

fn validate_gradle_home(custom_home: Option<&str>) -> Result<(), String> {
    let home_path = custom_home.map(std::path::Path::new);
    let home_dir = GradleCache::resolve_home_dir(home_path)
        .ok_or_else(|| "Could not determine Gradle cache home directory.".to_string())?;

    if !home_dir.exists() {
        return Err(common::concat_str!(
            "Gradle home directory does not exist: ",
            &home_dir.display().to_string()
        ));
    }

    Ok(())
}

fn print_fetch_summary(cache: &GradleCache, binaries: usize, sources: usize) {
    println!(
        "Done! Downloaded {} binaries and {} sources to Gradle cache ({}).",
        binaries,
        sources,
        cache.files_dir().display()
    );
}

pub async fn build_fetch_graph_with_progress(
    direct_deps: &[DependencyCoordinate],
    registry: &RepositoryRegistry,
    cache: &GradleCache,
    downloader: &Downloader,
    transitive: bool,
    offline: bool,
    progress: Option<&mut crate::progress::ResolveProgress>,
) -> SolvedGraph {
    if transitive {
        SolvedGraph::solve_with_progress(
            direct_deps,
            registry,
            cache,
            downloader,
            offline,
            progress,
        )
        .await
    } else {
        SolvedGraph {
            nodes: direct_deps
                .iter()
                .map(|d| GraphNode {
                    coordinate: d.clone(),
                    is_direct: true,
                    promoted_from: None,
                    children: Vec::new(),
                    cached_binary: cache.find_artifact(d, false),
                    cached_sources: cache.find_artifact(d, true),
                })
                .collect(),
            direct_count: direct_deps.len(),
            transitive_count: 0,
        }
    }
}

async fn download_fetch_artifacts(
    graph: &SolvedGraph,
    registry: &RepositoryRegistry,
    cache: &GradleCache,
    downloader: &Downloader,
    fetch_sources: bool,
    silent: bool,
) -> (usize, usize, usize) {
    let download_tasks = collect_missing_download_tasks(graph, fetch_sources);
    if download_tasks.is_empty() {
        return (0, 0, 0);
    }

    let (progress_handle, coordinator_handle) =
        crate::progress::start_progress_coordinator(download_tasks.len(), silent);
    let mut join_set =
        spawn_artifact_downloads(download_tasks, downloader, registry, cache, progress_handle);

    let mut downloaded_binaries = 0;
    let mut downloaded_sources = 0;
    let mut failed_binaries = 0;

    while let Some(res) = join_set.join_next().await {
        if let Ok((is_sources, outcome)) = res {
            match outcome {
                Ok(()) if is_sources => downloaded_sources += 1,
                Ok(()) => downloaded_binaries += 1,
                Err(_) if !is_sources => failed_binaries += 1,
                Err(_) => {}
            }
        }
    }

    let _ = coordinator_handle.await;
    (downloaded_binaries, downloaded_sources, failed_binaries)
}

fn collect_missing_download_tasks(
    graph: &SolvedGraph,
    fetch_sources: bool,
) -> Vec<(DependencyCoordinate, bool)> {
    let mut download_tasks = Vec::new();
    for node in &graph.nodes {
        if node.cached_binary.is_none() {
            download_tasks.push((node.coordinate.clone(), false));
        }
        if fetch_sources && node.cached_sources.is_none() {
            download_tasks.push((node.coordinate.clone(), true));
        }
    }
    download_tasks
}

fn spawn_artifact_downloads(
    download_tasks: Vec<(DependencyCoordinate, bool)>,
    downloader: &Downloader,
    registry: &RepositoryRegistry,
    cache: &GradleCache,
    progress_handle: crate::progress::ProgressHandle,
) -> tokio::task::JoinSet<(bool, Result<(), String>)> {
    let mut join_set = tokio::task::JoinSet::new();
    for (coord, is_sources) in download_tasks {
        let downloader_clone = downloader.clone();
        let registry_clone = registry.clone();
        let cache_clone = cache.clone();
        let label = if is_sources {
            common::concat_smol_str!(&coord.to_string(), " (sources)")
        } else {
            smol_str::SmolStr::new(coord.to_string())
        };
        let tracker = progress_handle.create_task(label, None);

        join_set.spawn(async move {
            let res = downloader_clone
                .download_artifact_with_tracker(
                    &registry_clone,
                    &coord,
                    is_sources,
                    &cache_clone,
                    Some(tracker),
                )
                .await;
            (is_sources, res.map(|_| ()))
        });
    }
    join_set
}

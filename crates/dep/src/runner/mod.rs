use crate::cache::GradleCache;
use crate::coordinate::{DependencyCoordinate, deduplicate_newest};
use crate::discovery;
use crate::downloader::Downloader;
use crate::graph::SolvedGraph;
use crate::renderer::TreeRenderer;
use crate::repo::RepositoryRegistry;
use cli::{
    DepArgs, DepListArgs, DepSubcommand, DepTreeArgs, render_dep_help, render_list_help,
    render_tree_help,
};
use common::concat_str;
use std::path::{Path, PathBuf};

mod fetch;
use fetch::run_fetch;

/// Main entry point for `kost dep` CLI command.
pub async fn run(arguments: &[String]) -> Result<(), String> {
    let dep_args = DepArgs::parse(arguments).map_err(|err| err.to_string())?;

    match dep_args.subcommand {
        DepSubcommand::Help => {
            print!("{}", render_dep_help());
            Ok(())
        }
        DepSubcommand::Fetch(fetch_args) => run_fetch(fetch_args).await,
        DepSubcommand::List(list_args) => run_list(list_args).await,
        DepSubcommand::Tree(tree_args) => run_tree(tree_args).await,
    }
}

pub(crate) struct DepContext {
    pub(crate) project_path: PathBuf,
    pub(crate) cache: GradleCache,
    pub(crate) registry: RepositoryRegistry,
    pub(crate) direct_deps: Vec<DependencyCoordinate>,
}

pub(crate) fn prepare_context(
    project_dir: Option<&str>,
    gradle_home: Option<&str>,
    extra_repos: &[String],
    coordinates: &[String],
) -> Result<DepContext, String> {
    let project_path = resolve_project_dir(project_dir);
    let custom_home = gradle_home.map(Path::new);
    let cache = GradleCache::new(custom_home)
        .ok_or_else(|| "Could not determine Gradle cache home directory.".to_string())?;

    let (catalogs, scripts) = discovery::discover_gradle_files(&project_path);
    let (direct_deps, registry) =
        resolve_deps_and_registry(coordinates, &catalogs, &scripts, extra_repos)?;
    Ok(DepContext {
        project_path,
        cache,
        registry,
        direct_deps,
    })
}

fn resolve_deps_and_registry(
    coordinates: &[String],
    catalogs: &[PathBuf],
    scripts: &[PathBuf],
    extra_repos: &[String],
) -> Result<(Vec<DependencyCoordinate>, RepositoryRegistry), String> {
    let explicit_coords = !coordinates.is_empty();
    let (discovered_deps, repo_result) =
        discovery::collect_project_dependencies_and_repos(catalogs, scripts, !explicit_coords)
            .map_err(|err| err.to_string())?;

    let mut registry = RepositoryRegistry::new();
    registry.apply_repositories_result(repo_result);
    for repo_url in extra_repos {
        registry.add_url(repo_url);
    }

    if registry.is_empty() {
        return Err(
            "No repositories found in project build scripts. Configure a repositories block or provide --maven-repo.".to_string(),
        );
    }

    if explicit_coords {
        let mut deps = Vec::with_capacity(coordinates.len());
        for coord_str in coordinates {
            let coord = DependencyCoordinate::parse(coord_str).ok_or_else(|| {
                concat_str!(
                    "Invalid dependency coordinate '",
                    coord_str,
                    "'. Expected group:artifact:version."
                )
            })?;
            deps.push(coord);
        }
        return Ok((deduplicate_newest(deps), registry));
    }

    if discovered_deps.is_empty() {
        return Err("No dependencies found in project build scripts or version catalogs. Provide coordinates explicitly.".to_string());
    }
    Ok((discovered_deps, registry))
}

async fn run_list(args: DepListArgs) -> Result<(), String> {
    if args.help {
        print!("{}", render_list_help());
        return Ok(());
    }

    let ctx = prepare_context(
        args.project_dir.as_deref(),
        args.gradle_home.as_deref(),
        &args.repositories,
        &args.coordinates,
    )?;
    let downloader = Downloader::default();

    let mut progress = if !args.offline && !args.direct {
        Some(crate::progress::ResolveProgress::default_for_stderr())
    } else {
        None
    };

    let query_cache = ctx.cache.clone().read_only();
    let graph = fetch::build_fetch_graph_with_progress(
        &ctx.direct_deps,
        &ctx.registry,
        &query_cache,
        &downloader,
        !args.direct,
        args.offline,
        progress.as_mut(),
    )
    .await;

    let colorize = common::should_colorize_stdout();
    for node in &graph.nodes {
        let is_cached = node.cached_binary.is_some();
        if args.cached_only && !is_cached {
            continue;
        }
        if args.missing_only && is_cached {
            continue;
        }
        let status = TreeRenderer::format_status_tag(is_cached, colorize);
        println!("{} {}", node.coordinate, status);
    }

    Ok(())
}

async fn run_tree(args: DepTreeArgs) -> Result<(), String> {
    if args.help {
        print!("{}", render_tree_help());
        return Ok(());
    }

    let ctx = prepare_context(
        args.project_dir.as_deref(),
        args.gradle_home.as_deref(),
        &args.repositories,
        &args.coordinates,
    )?;
    let downloader = Downloader::default();

    let mut progress = if !args.offline {
        Some(crate::progress::ResolveProgress::default_for_stderr())
    } else {
        None
    };

    let query_cache = ctx.cache.clone().read_only();
    let graph = SolvedGraph::solve_with_progress(
        &ctx.direct_deps,
        &ctx.registry,
        &query_cache,
        &downloader,
        args.offline,
        progress.as_mut(),
    )
    .await;

    print!(
        "{}",
        TreeRenderer::render_tree(
            &graph,
            Some(&ctx.project_path),
            args.depth,
            args.filter.as_deref(),
        )
    );

    Ok(())
}

fn resolve_project_dir(custom_path: Option<&str>) -> PathBuf {
    custom_path
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

use resolver::cache_resolver::ResolvedArtifact;
use std::path::{Path, PathBuf};

use crate::gradle::config::GradleConfig;
use crate::gradle::dependency::DependencyCoordinate;

pub struct GradleCacheResolver;

impl GradleCacheResolver {
    /// Resolve a collection of dependencies to their local jar/aar and sources.jar files.
    pub fn resolve_all(
        deps: &[DependencyCoordinate],
        config: &GradleConfig,
    ) -> Vec<ResolvedArtifact> {
        let gradle_home = config.resolve_gradle_user_home();
        let m2_repo = config.resolve_m2_repository();
        resolver::GradleCacheResolver::resolve_all_with_paths(
            deps,
            gradle_home.as_deref(),
            m2_repo.as_deref(),
            &config.extra_dependency_paths,
        )
    }

    /// Resolve a single dependency coordinate across Gradle cache, M2 repo, and extra paths.
    /// Prefers `-desktop` suffix -> `-jvm` suffix -> no suffix for Kotlin Multiplatform libraries.
    pub fn resolve_single(
        dep: &DependencyCoordinate,
        gradle_home: Option<&Path>,
        m2_repo: Option<&Path>,
        extra_paths: &[PathBuf],
    ) -> Option<ResolvedArtifact> {
        resolver::GradleCacheResolver::resolve_single(dep, gradle_home, m2_repo, extra_paths)
    }

    /// Discover available Kotlin Stdlib in Gradle cache or config paths.
    pub fn find_kotlin_stdlib(
        preferred_version: Option<&str>,
        config: &GradleConfig,
    ) -> Option<ResolvedArtifact> {
        let gradle_home = config.resolve_gradle_user_home();
        let m2_repo = config.resolve_m2_repository();
        resolver::GradleCacheResolver::find_kotlin_stdlib_with_paths(
            preferred_version,
            gradle_home.as_deref(),
            m2_repo.as_deref(),
            &config.extra_dependency_paths,
        )
    }
}

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::path::{Path, PathBuf};

use crate::gradle::config::GradleConfig;
use crate::gradle::dependency::DependencyCoordinate;
use crate::gradle::version::ArtifactVersion;

/// Resolved local file paths for a dependency coordinate.
#[derive(Debug, Clone)]
pub struct ResolvedArtifact {
    pub coordinate: DependencyCoordinate,
    pub binary_path: PathBuf,
    pub sources_path: Option<PathBuf>,
}

pub fn normalize_archive_path(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}

impl ResolvedArtifact {
    /// Return the target archive path that will be indexed based on `prefer_sources`.
    pub fn target_path(&self, prefer_sources: bool) -> &Path {
        if prefer_sources && let Some(ref src) = self.sources_path {
            return src.as_path();
        }
        self.binary_path.as_path()
    }

    /// Check if this artifact's target or source/binary archive has already been processed.
    pub fn is_already_processed(
        &self,
        processed: &ahash::AHashSet<PathBuf>,
        prefer_sources: bool,
    ) -> bool {
        let target = normalize_archive_path(self.target_path(prefer_sources));
        if processed.contains(&target) {
            return true;
        }
        let bin = normalize_archive_path(&self.binary_path);
        if processed.contains(&bin) {
            return true;
        }
        if let Some(ref src) = self.sources_path {
            let s = normalize_archive_path(src);
            if processed.contains(&s) {
                return true;
            }
        }
        false
    }

    /// Record this artifact's target, binary, and sources archive paths into the processed set.
    pub fn record_processed(&self, processed: &mut ahash::AHashSet<PathBuf>, prefer_sources: bool) {
        processed.insert(normalize_archive_path(self.target_path(prefer_sources)));
        processed.insert(normalize_archive_path(&self.binary_path));
        if let Some(ref src) = self.sources_path {
            processed.insert(normalize_archive_path(src));
        }
    }
}

/// Pre-computed context for resolving artifacts across Gradle cache and local repositories.
pub struct CacheResolverContext<'a> {
    files_2_1: Option<PathBuf>,
    m2_repo: Option<PathBuf>,
    extra_paths: &'a [PathBuf],
}

impl<'a> CacheResolverContext<'a> {
    pub fn new(
        gradle_home: Option<&Path>,
        m2_repo: Option<&Path>,
        extra_paths: &'a [PathBuf],
    ) -> Self {
        let files_2_1 = gradle_home
            .map(|gh| gh.join("caches").join("modules-2").join("files-2.1"))
            .filter(|p| p.is_dir());
        let m2_repo = m2_repo.filter(|p| p.is_dir()).map(|p| p.to_path_buf());
        Self {
            files_2_1,
            m2_repo,
            extra_paths,
        }
    }

    pub fn resolve_single(&self, dep: &DependencyCoordinate) -> Option<ResolvedArtifact> {
        for cand_name in dep.kmp_candidate_names() {
            let mut cand_dep = dep.clone();
            cand_dep.name = smol_str::SmolStr::new(&cand_name);

            if let Some(res) = self.resolve_exact(&cand_dep) {
                return Some(res);
            }
        }
        None
    }

    fn resolve_exact(&self, dep: &DependencyCoordinate) -> Option<ResolvedArtifact> {
        // Extra configured search paths
        for extra in self.extra_paths {
            if let Some(res) = GradleCacheResolver::search_directory(dep, extra) {
                return Some(res);
            }
        }

        // Gradle cache
        if let Some(ref files_2_1) = self.files_2_1 {
            let name_dir = files_2_1.join(dep.group.as_str()).join(dep.name.as_str());
            if name_dir.is_dir() {
                let ver_str = dep.version.as_ref().map(|v| v.raw()).unwrap_or("");
                let ver_dir_opt = if !ver_str.is_empty() {
                    Some(name_dir.join(ver_str))
                } else {
                    find_highest_version_dir(&name_dir)
                };

                if let Some(ver_dir) = ver_dir_opt
                    && ver_dir.is_dir()
                    && let Some(res) = GradleCacheResolver::search_gradle_version_dir(dep, &ver_dir)
                {
                    return Some(res);
                }
            }
        }

        // Maven local ~/.m2/repository
        if let Some(ref m2) = self.m2_repo {
            let group_path = dep.group.replace('.', "/");
            let ver_str = dep.version.as_ref().map(|v| v.raw()).unwrap_or("");
            let artifact_dir = m2.join(&group_path).join(dep.name.as_str()).join(ver_str);
            if artifact_dir.is_dir()
                && let Some(res) = GradleCacheResolver::search_m2_dir(dep, &artifact_dir)
            {
                return Some(res);
            }
        }

        None
    }
}

pub struct GradleCacheResolver;

impl GradleCacheResolver {
    /// Resolve a collection of dependencies to their local jar/aar and sources.jar files.
    pub fn resolve_all(
        deps: &[DependencyCoordinate],
        config: &GradleConfig,
    ) -> Vec<ResolvedArtifact> {
        let gradle_home = config.resolve_gradle_user_home();
        let m2_repo = config.resolve_m2_repository();
        let context = CacheResolverContext::new(
            gradle_home.as_deref(),
            m2_repo.as_deref(),
            &config.extra_dependency_paths,
        );

        deps.par_iter()
            .filter_map(|dep| context.resolve_single(dep))
            .collect()
    }

    /// Resolve a single dependency coordinate across Gradle cache, M2 repo, and extra paths.
    /// Prefers -desktop suffix -> -jvm suffix -> no suffix for Kotlin Multiplatform libraries.
    pub fn resolve_single(
        dep: &DependencyCoordinate,
        gradle_home: Option<&Path>,
        m2_repo: Option<&Path>,
        extra_paths: &[PathBuf],
    ) -> Option<ResolvedArtifact> {
        let context = CacheResolverContext::new(gradle_home, m2_repo, extra_paths);
        context.resolve_single(dep)
    }

    fn search_gradle_version_dir(
        dep: &DependencyCoordinate,
        ver_dir: &Path,
    ) -> Option<ResolvedArtifact> {
        let mut binary_path = None;
        let mut sources_path = None;

        let bin_name = dep.artifact_file_name(false);
        let src_name = dep.artifact_file_name(true);

        let Ok(hash_entries) = std::fs::read_dir(ver_dir) else {
            return None;
        };
        for hash_entry in hash_entries.flatten() {
            let Ok(file_type) = hash_entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let Ok(file_entries) = std::fs::read_dir(hash_entry.path()) else {
                continue;
            };
            for file_entry in file_entries.flatten() {
                let name = file_entry.file_name();
                let Some(name_str) = name.to_str() else {
                    continue;
                };
                let is_jar_or_aar = (name_str.ends_with(".jar") || name_str.ends_with(".aar"))
                    && !name_str.ends_with("-sources.jar")
                    && !name_str.ends_with("-javadoc.jar");

                if name_str == bin_name || (binary_path.is_none() && is_jar_or_aar) {
                    binary_path = Some(file_entry.path());
                } else if name_str == src_name
                    || (sources_path.is_none() && name_str.ends_with("-sources.jar"))
                {
                    sources_path = Some(file_entry.path());
                }
            }
        }

        let effective_bin = binary_path.or_else(|| sources_path.clone())?;
        let mut coord = dep.clone();
        if coord.version.is_none()
            && let Some(ver_str) = ver_dir.file_name().and_then(|n| n.to_str())
        {
            coord.version = Some(crate::gradle::version::ArtifactVersion::parse(ver_str));
        }
        Some(ResolvedArtifact {
            coordinate: coord,
            binary_path: effective_bin,
            sources_path,
        })
    }

    fn search_m2_dir(dep: &DependencyCoordinate, dir: &Path) -> Option<ResolvedArtifact> {
        let bin_name = dep.artifact_file_name(false);
        let src_name = dep.artifact_file_name(true);

        let bin_file = dir.join(&bin_name);
        let src_file = dir.join(&src_name);

        if bin_file.is_file() {
            Some(ResolvedArtifact {
                coordinate: dep.clone(),
                binary_path: bin_file,
                sources_path: if src_file.is_file() {
                    Some(src_file)
                } else {
                    None
                },
            })
        } else if src_file.is_file() {
            Some(ResolvedArtifact {
                coordinate: dep.clone(),
                binary_path: src_file.clone(),
                sources_path: Some(src_file),
            })
        } else {
            None
        }
    }

    fn search_directory(dep: &DependencyCoordinate, root: &Path) -> Option<ResolvedArtifact> {
        let bin_name = dep.artifact_file_name(false);
        let src_name = dep.artifact_file_name(true);

        let mut binary = None;
        let mut sources = None;

        for entry in walkdir_simple(root) {
            if let Some(name) = entry.file_name().and_then(|n| n.to_str()) {
                if name == bin_name {
                    binary = Some(entry.clone());
                } else if name == src_name {
                    sources = Some(entry);
                }
            }
        }

        let effective_bin = binary.or_else(|| sources.clone())?;
        Some(ResolvedArtifact {
            coordinate: dep.clone(),
            binary_path: effective_bin,
            sources_path: sources,
        })
    }

    /// Discover available Kotlin Stdlib in Gradle cache or config paths
    pub fn find_kotlin_stdlib(
        preferred_version: Option<&str>,
        config: &GradleConfig,
    ) -> Option<ResolvedArtifact> {
        let stdlib_dep =
            DependencyCoordinate::new("org.jetbrains.kotlin", "kotlin-stdlib", preferred_version);

        let gradle_home = config.resolve_gradle_user_home();
        let m2_repo = config.resolve_m2_repository();

        Self::resolve_single(
            &stdlib_dep,
            gradle_home.as_deref(),
            m2_repo.as_deref(),
            &config.extra_dependency_paths,
        )
    }
}

fn find_highest_version_dir(group_dir: &Path) -> Option<PathBuf> {
    let mut versions: Vec<(ArtifactVersion, PathBuf)> = Vec::new();

    if let Ok(entries) = std::fs::read_dir(group_dir) {
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && let Some(name) = entry.file_name().to_str()
            {
                versions.push((ArtifactVersion::parse(name), entry.path()));
            }
        }
    }

    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions.into_iter().next().map(|(_, p)| p)
}

fn walkdir_simple(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() {
                    result.push(path);
                }
            }
        }
    }

    result
}

use std::path::Path;

use crate::gradle::cache_resolver::ResolvedArtifact;
use crate::gradle::config::GradleConfig;
use crate::gradle::dependency::DependencyCoordinate;

/// Resolver for standalone Kotlin compiler (`kotlinc`) installation and standard library.
pub struct KotlincResolver;

impl KotlincResolver {
    /// Discover `kotlin-stdlib-sources.jar` from kotlinc installation.
    pub fn find_kotlinc_stdlib(config: &GradleConfig) -> Option<ResolvedArtifact> {
        // 1. Explicitly configured path in GradleConfig
        if config.kotlinc_home.is_some() {
            let home = config.resolve_kotlinc_home()?;
            return Self::check_kotlinc_home(&home);
        }

        // 2. Environment variable home (KOTLIN_HOME / KOTLINC_HOME)
        if let Some(home) = config.resolve_kotlinc_home()
            && let Some(art) = Self::check_kotlinc_home(&home)
        {
            return Some(art);
        }

        // 3. Discover kotlinc binary from PATH and inspect ../lib/
        Self::find_from_path_env()
    }

    /// Check if home directory contains lib/kotlin-stdlib-sources.jar
    pub fn check_kotlinc_home(home: &Path) -> Option<ResolvedArtifact> {
        let lib_dir = if home.join("lib").is_dir() {
            home.join("lib")
        } else if home.is_dir() && home.file_name().is_some_and(|n| n == "lib") {
            home.to_path_buf()
        } else {
            return None;
        };

        let sources_jar = lib_dir.join("kotlin-stdlib-sources.jar");
        if !sources_jar.is_file() {
            return None;
        }

        Some(ResolvedArtifact {
            coordinate: DependencyCoordinate::new("org.jetbrains.kotlin", "kotlin-stdlib", None),
            binary_path: sources_jar.clone(),
            sources_path: Some(sources_jar),
        })
    }

    fn find_from_path_env() -> Option<ResolvedArtifact> {
        let path_os = std::env::var_os("PATH")?;
        for entry in std::env::split_paths(&path_os) {
            for bin_name in &["kotlinc", "kotlinc.bat", "kotlinc.cmd", "kotlinc.exe"] {
                let bin_path = entry.join(bin_name);
                if bin_path.is_file() {
                    if let Some(parent) = entry.parent()
                        && let Some(art) = Self::check_kotlinc_home(parent)
                    {
                        return Some(art);
                    }
                    if let Ok(canon) = std::fs::canonicalize(&bin_path)
                        && let Some(canon_home) = canon.parent().and_then(|p| p.parent())
                        && let Some(art) = Self::check_kotlinc_home(canon_home)
                    {
                        return Some(art);
                    }
                }
            }
        }
        None
    }
}

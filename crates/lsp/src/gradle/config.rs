use std::path::{Path, PathBuf};

/// Configuration options for Gradle and Android SDK resolution.
/// Allows passing custom paths or overriding auto-discovery behavior for testing and environments.
#[derive(Debug, Clone, Default)]
pub struct GradleConfig {
    /// Custom path to Gradle user home (e.g. `~/.gradle` or `$GRADLE_USER_HOME`).
    pub gradle_user_home: Option<PathBuf>,
    /// Custom path to Android SDK root (e.g. `local.properties` sdk.dir, `$ANDROID_HOME`, `$ANDROID_SDK_ROOT`).
    pub android_sdk_root: Option<PathBuf>,
    /// Custom path to Kotlin compiler root or kotlinc installation (e.g. `$KOTLIN_HOME`, `$KOTLINC_HOME`).
    pub kotlinc_home: Option<PathBuf>,
    /// Custom path to local Maven repository (e.g. `~/.m2/repository`).
    pub m2_repository: Option<PathBuf>,
    /// Additional directories containing .jar or .aar dependencies.
    pub extra_dependency_paths: Vec<PathBuf>,
    /// Whether to prioritize -sources.jar when available for full fidelity symbol extraction.
    pub prefer_sources: bool,
    /// Verbosity level for Gradle and artifact indexing logging.
    pub indexing_logging: crate::config::IndexingLogLevel,
}

impl GradleConfig {
    pub fn new() -> Self {
        Self {
            gradle_user_home: None,
            android_sdk_root: None,
            kotlinc_home: None,
            m2_repository: None,
            extra_dependency_paths: Vec::new(),
            prefer_sources: true,
            indexing_logging: crate::config::IndexingLogLevel::Default,
        }
    }

    pub fn with_gradle_user_home(mut self, path: impl AsRef<Path>) -> Self {
        self.gradle_user_home = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_android_sdk_root(mut self, path: impl AsRef<Path>) -> Self {
        self.android_sdk_root = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_kotlinc_home(mut self, path: impl AsRef<Path>) -> Self {
        self.kotlinc_home = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_m2_repository(mut self, path: impl AsRef<Path>) -> Self {
        self.m2_repository = Some(path.as_ref().to_path_buf());
        self
    }

    pub fn with_extra_dependency_path(mut self, path: impl AsRef<Path>) -> Self {
        self.extra_dependency_paths
            .push(path.as_ref().to_path_buf());
        self
    }

    pub fn with_indexing_logging(mut self, level: crate::config::IndexingLogLevel) -> Self {
        self.indexing_logging = level;
        self
    }

    /// Resolve effective kotlinc home path
    pub fn resolve_kotlinc_home(&self) -> Option<PathBuf> {
        if let Some(ref p) = self.kotlinc_home {
            return if p.exists() { Some(p.clone()) } else { None };
        }
        for env_var in &["KOTLIN_HOME", "KOTLINC_HOME", "KOTLIN_ROOT"] {
            if let Ok(val) = std::env::var(env_var) {
                let p = PathBuf::from(val);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        None
    }

    /// Resolve effective Gradle user home path
    pub fn resolve_gradle_user_home(&self) -> Option<PathBuf> {
        if let Some(ref p) = self.gradle_user_home
            && p.exists()
        {
            return Some(p.clone());
        }
        if let Ok(env_home) = std::env::var("GRADLE_USER_HOME") {
            let p = PathBuf::from(env_home);
            if p.exists() {
                return Some(p);
            }
        }
        dirs_home()
            .map(|h| h.join(".gradle"))
            .filter(|p| p.exists())
    }

    /// Resolve effective Maven local repository path
    pub fn resolve_m2_repository(&self) -> Option<PathBuf> {
        if let Some(ref p) = self.m2_repository
            && p.exists()
        {
            return Some(p.clone());
        }
        dirs_home()
            .map(|h| h.join(".m2").join("repository"))
            .filter(|p| p.exists())
    }
}

pub(crate) fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

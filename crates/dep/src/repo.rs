use common::concat_str;
use regex::Regex;
use std::sync::OnceLock;

use crate::coordinate::DependencyCoordinate;

/// Represents a Maven repository with an ID and base URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repository {
    pub id: String,
    pub url: String,
}

impl Repository {
    pub fn new(id: impl Into<String>, url: impl Into<String>) -> Self {
        let mut clean_url = url.into().trim().to_string();
        if !clean_url.ends_with('/') {
            clean_url.push('/');
        }
        Self {
            id: id.into(),
            url: clean_url,
        }
    }

    pub fn maven_central() -> Self {
        Self::new("mavenCentral", "https://repo1.maven.org/maven2/")
    }

    pub fn google() -> Self {
        Self::new("google", "https://maven.google.com/")
    }

    pub fn gradle_plugin_portal() -> Self {
        Self::new("gradlePluginPortal", "https://plugins.gradle.org/m2/")
    }

    pub fn jitpack() -> Self {
        Self::new("jitpack", "https://jitpack.io/")
    }

    /// Construct URL for an artifact binary or source jar
    pub fn artifact_url(&self, coord: &DependencyCoordinate, is_sources: bool) -> String {
        let group_path = coord.group.replace('.', "/");
        let file_name = coord.artifact_file_name(is_sources);
        let ver = coord
            .version
            .as_ref()
            .map(|v| v.raw())
            .unwrap_or("unspecified");

        concat_str!(
            &self.url,
            &group_path,
            "/",
            &coord.name,
            "/",
            ver,
            "/",
            &file_name
        )
    }

    /// Construct URL for POM file
    pub fn pom_url(&self, coord: &DependencyCoordinate) -> String {
        let group_path = coord.group.replace('.', "/");
        let file_name = coord.pom_file_name();
        let ver = coord
            .version
            .as_ref()
            .map(|v| v.raw())
            .unwrap_or("unspecified");

        concat_str!(
            &self.url,
            &group_path,
            "/",
            &coord.name,
            "/",
            ver,
            "/",
            &file_name
        )
    }

    /// Construct URL for Gradle Module Metadata (.module)
    pub fn module_url(&self, coord: &DependencyCoordinate) -> String {
        let group_path = coord.group.replace('.', "/");
        let file_name = coord.module_file_name();
        let ver = coord
            .version
            .as_ref()
            .map(|v| v.raw())
            .unwrap_or("unspecified");

        concat_str!(
            &self.url,
            &group_path,
            "/",
            &coord.name,
            "/",
            ver,
            "/",
            &file_name
        )
    }
}

/// Registry of repositories to query in priority order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRegistry {
    pub repositories: Vec<Repository>,
}

impl Default for RepositoryRegistry {
    fn default() -> Self {
        Self {
            repositories: vec![
                Repository::google(),
                Repository::maven_central(),
                Repository::gradle_plugin_portal(),
                Repository::jitpack(),
            ],
        }
    }
}

impl RepositoryRegistry {
    pub fn new() -> Self {
        Self {
            repositories: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::default()
    }

    pub fn add(&mut self, repo: Repository) {
        if !self.repositories.iter().any(|r| r.url == repo.url) {
            self.repositories.push(repo);
        }
    }

    pub fn add_url(&mut self, url: &str) {
        let id = concat_str!("repo_", &(self.repositories.len() + 1).to_string());
        self.add(Repository::new(id, url));
    }

    /// Parse repository definitions from Gradle build script / settings content.
    pub fn parse_from_script(&mut self, content: &str) {
        let cleaned = crate::discovery::strip_script_comments(content);
        for url in extract_maven_urls(&cleaned) {
            self.add_url(&url);
        }

        if cleaned.contains("google()") {
            self.add(Repository::google());
        }
        if cleaned.contains("mavenCentral()") {
            self.add(Repository::maven_central());
        }
        if cleaned.contains("gradlePluginPortal()") {
            self.add(Repository::gradle_plugin_portal());
        }
        if cleaned.contains("mavenLocal()")
            && let Some(home) = crate::cache::dirs_home()
        {
            let m2 = home.join(".m2").join("repository");
            if m2.is_dir() {
                let m2_str = m2.to_string_lossy().replace('\\', "/");
                let prefix = if m2_str.starts_with('/') {
                    "file://"
                } else {
                    "file:///"
                };
                let url = concat_str!(prefix, &m2_str, "/");
                self.add(Repository::new("mavenLocal", &url));
            }
        }
    }
}

fn extract_maven_urls(content: &str) -> Vec<String> {
    static MAVEN_CALL_RE: OnceLock<Regex> = OnceLock::new();
    let maven_call_re = MAVEN_CALL_RE.get_or_init(|| {
        Regex::new(r#"(?s)maven\s*(?:\(\s*(?:url\s*=\s*)?(?:uri\s*\(\s*)?["']((?:https?|file)://[^"']+)["']|\{[^}]*?(?:url\s*=\s*|url\s+|setUrl\s*\(\s*)(?:uri\s*\(\s*)?["']((?:https?|file)://[^"']+)["'])"#).unwrap()
    });

    let mut urls = Vec::new();
    for cap in maven_call_re.captures_iter(content) {
        if let Some(m) = cap.get(1).or_else(|| cap.get(2)) {
            urls.push(m.as_str().to_string());
        }
    }
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repository_url_construction() {
        let repo = Repository::maven_central();
        let coord =
            DependencyCoordinate::parse("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.8.0")
                .unwrap();

        let jar_url = repo.artifact_url(&coord, false);
        assert_eq!(
            jar_url,
            "https://repo1.maven.org/maven2/org/jetbrains/kotlinx/kotlinx-coroutines-core/1.8.0/kotlinx-coroutines-core-1.8.0.jar"
        );

        let pom_url = repo.pom_url(&coord);
        assert_eq!(
            pom_url,
            "https://repo1.maven.org/maven2/org/jetbrains/kotlinx/kotlinx-coroutines-core/1.8.0/kotlinx-coroutines-core-1.8.0.pom"
        );

        let module_url = repo.module_url(&coord);
        assert_eq!(
            module_url,
            "https://repo1.maven.org/maven2/org/jetbrains/kotlinx/kotlinx-coroutines-core/1.8.0/kotlinx-coroutines-core-1.8.0.module"
        );
    }
}

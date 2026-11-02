use ahash::AHashMap;
use common::concat_str;

use crate::ast::{
    DeclaredRepository, ParseError, RepositoriesResult, ScriptDialect,
    groovy_repo::parse_groovy_repositories_with_props,
    kotlin_repo::parse_kotlin_repositories_with_props,
};
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

    pub fn jcenter() -> Self {
        Self::new("jcenter", "https://jcenter.bintray.com/")
    }

    /// Construct URL for an artifact binary or source jar.
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

    /// Construct URL for POM file.
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

    /// Construct URL for Gradle Module Metadata (`.module`).
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

    /// Construct URL for Maven metadata (`maven-metadata.xml`).
    pub fn maven_metadata_url(&self, group: &str, artifact: &str) -> String {
        let group_path = group.replace('.', "/");
        concat_str!(&self.url, &group_path, "/", artifact, "/maven-metadata.xml")
    }
}

/// Registry of repositories to query in priority order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepositoryRegistry {
    pub repositories: Vec<Repository>,
}

impl RepositoryRegistry {
    pub fn new() -> Self {
        Self {
            repositories: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.repositories.is_empty()
    }

    pub fn add(&mut self, repo: Repository) {
        if !self
            .repositories
            .iter()
            .any(|existing| existing.url == repo.url)
        {
            self.repositories.push(repo);
        }
    }

    pub fn add_url(&mut self, url: &str) {
        let id = concat_str!("repo_", &(self.repositories.len() + 1).to_string());
        self.add(Repository::new(id, url));
    }

    /// Parse repository definitions from Gradle build script with explicit dialect.
    pub fn parse_from_script(
        &mut self,
        content: &str,
        dialect: ScriptDialect,
    ) -> Result<(), ParseError> {
        self.parse_from_script_with_props(content, dialect, &AHashMap::default())
    }

    /// Parse repository definitions from Gradle build script with inherited properties.
    pub fn parse_from_script_with_props(
        &mut self,
        content: &str,
        dialect: ScriptDialect,
        properties: &AHashMap<String, String>,
    ) -> Result<(), ParseError> {
        let repo_result = match dialect {
            ScriptDialect::Kotlin => parse_kotlin_repositories_with_props(content, properties)?,
            ScriptDialect::Groovy => parse_groovy_repositories_with_props(content, properties)?,
        };
        self.apply_repositories_result(repo_result);
        Ok(())
    }

    pub fn apply_repositories_result(&mut self, repo_result: RepositoriesResult) {
        if !repo_result.entries.is_empty() || !repo_result.plugin_entries.is_empty() {
            for entry in repo_result.ordered_entries() {
                match entry {
                    DeclaredRepository::Google => self.add(Repository::google()),
                    DeclaredRepository::MavenCentral => self.add(Repository::maven_central()),
                    DeclaredRepository::GradlePluginPortal => {
                        self.add(Repository::gradle_plugin_portal());
                    }
                    DeclaredRepository::MavenLocal => self.add_maven_local(),
                    DeclaredRepository::JCenter => self.add(Repository::jcenter()),
                    DeclaredRepository::MavenUrl(url) => self.add_url(url),
                }
            }
            return;
        }

        for url in repo_result.urls {
            self.add_url(&url);
        }
        if repo_result.google {
            self.add(Repository::google());
        }
        if repo_result.maven_central {
            self.add(Repository::maven_central());
        }
        if repo_result.gradle_plugin_portal {
            self.add(Repository::gradle_plugin_portal());
        }
        if repo_result.maven_local {
            self.add_maven_local();
        }
        if repo_result.jcenter {
            self.add(Repository::jcenter());
        }
    }

    fn add_maven_local(&mut self) {
        if let Some(home) = crate::cache::dirs_home() {
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

        let meta_url = repo.maven_metadata_url(&coord.group, &coord.name);
        assert_eq!(
            meta_url,
            "https://repo1.maven.org/maven2/org/jetbrains/kotlinx/kotlinx-coroutines-core/maven-metadata.xml"
        );
    }
}

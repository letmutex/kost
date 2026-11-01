pub mod interpolate;
pub mod parser;

use ahash::AHashMap;
use smol_str::SmolStr;

use crate::cache::GradleCache;
use crate::coordinate::DependencyCoordinate;

/// A parsed dependency from POM `<dependencies>` or `<dependencyManagement>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PomDependency {
    pub group: SmolStr,
    pub name: SmolStr,
    pub version: Option<String>,
    pub scope: Option<String>,
    pub optional: bool,
    pub exclusions: Vec<(SmolStr, SmolStr)>,
}

/// A parsed `<parent>` block from POM.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PomParent {
    pub group: SmolStr,
    pub name: SmolStr,
    pub version: String,
}

/// Parsed POM document model.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PomModel {
    pub parent: Option<PomParent>,
    pub group: Option<SmolStr>,
    pub name: Option<SmolStr>,
    pub version: Option<String>,
    pub packaging: SmolStr,
    pub properties: AHashMap<String, String>,
    pub dependencies: Vec<PomDependency>,
    pub dependency_management: Vec<PomDependency>,
}

impl PomModel {
    pub fn parse(xml_content: &str) -> Result<Self, String> {
        parser::parse_pom(xml_content)
    }

    /// Interpolates variable references `${prop}` using POM properties and inherited variables.
    pub fn interpolate(&mut self, inherited_properties: &AHashMap<String, String>) {
        interpolate::interpolate_pom(self, inherited_properties);
    }

    /// Inherits properties and dependency management from a resolved parent POM model.
    pub fn inherit_from_parent(&mut self, parent: &PomModel) {
        for (key, value) in &parent.properties {
            self.properties
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
        for dep in &parent.dependency_management {
            if !self
                .dependency_management
                .iter()
                .any(|existing| existing.group == dep.group && existing.name == dep.name)
            {
                self.dependency_management.push(dep.clone());
            }
        }
    }

    /// Resolves parent POM inheritance up to 4 levels deep from the local Gradle cache.
    pub fn resolve_parent_from_cache(&mut self, cache: &GradleCache) {
        resolve_parent_pom_depth(self, cache, 0);
    }

    /// Extract resolved direct dependencies (excluding optional and test-scoped).
    pub fn direct_dependencies(&self) -> Vec<DependencyCoordinate> {
        self.dependencies
            .iter()
            .filter(|dep| {
                !dep.optional
                    && !dep.group.is_empty()
                    && !dep.name.is_empty()
                    && !matches!(dep.scope.as_deref(), Some("test" | "provided" | "system"))
            })
            .map(|dep| {
                DependencyCoordinate::new(
                    dep.group.clone(),
                    dep.name.clone(),
                    dep.version.as_deref(),
                )
            })
            .collect()
    }
}

fn resolve_parent_pom_depth(pom: &mut PomModel, cache: &GradleCache, depth: usize) {
    if depth >= 4 {
        return;
    }
    if let Some(ref parent_info) = pom.parent
        && let parent_coord = DependencyCoordinate::new(
            parent_info.group.clone(),
            parent_info.name.clone(),
            Some(&parent_info.version),
        )
        && let Some(parent_path) = cache.find_pom(&parent_coord)
        && let Ok(content) = std::fs::read_to_string(&parent_path)
        && let Ok(mut parent_pom) = PomModel::parse(&content)
    {
        resolve_parent_pom_depth(&mut parent_pom, cache, depth + 1);
        pom.inherit_from_parent(&parent_pom);
    }
}

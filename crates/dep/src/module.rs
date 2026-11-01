use serde::Deserialize;
use smol_str::SmolStr;

use crate::coordinate::DependencyCoordinate;

#[derive(Debug, Clone, Deserialize)]
pub struct GradleModuleMetadata {
    #[serde(rename = "formatVersion")]
    pub format_version: Option<String>,
    pub component: Option<ModuleComponent>,
    pub variants: Option<Vec<ModuleVariant>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleComponent {
    pub group: String,
    pub module: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleVariant {
    pub name: String,
    pub dependencies: Option<Vec<ModuleDependency>>,
    pub files: Option<Vec<ModuleFile>>,
    #[serde(rename = "available-at")]
    pub available_at: Option<ModuleAvailableAt>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleAvailableAt {
    pub group: String,
    pub module: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleDependency {
    pub group: String,
    pub module: String,
    pub version: Option<ModuleVersionConstraint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleVersionConstraint {
    pub requires: Option<String>,
    pub prefers: Option<String>,
    pub strictly: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModuleFile {
    pub name: String,
    pub url: String,
    pub size: Option<u64>,
    pub sha1: Option<String>,
}

impl GradleModuleMetadata {
    pub fn parse(json_content: &str) -> Result<Self, String> {
        serde_json::from_str(json_content)
            .map_err(|err| common::concat_str!("Module JSON parse error: ", &err.to_string()))
    }

    /// Extract dependencies from preferred runtime/api variants (JVM, Desktop, Android, common).
    pub fn resolved_dependencies(&self) -> Vec<DependencyCoordinate> {
        let mut deps = Vec::new();
        let Some(ref variants) = self.variants else {
            return deps;
        };

        let selected_variant = select_preferred_variant(variants);

        if let Some(variant) = selected_variant {
            if let Some(ref avail) = variant.available_at
                && !avail.group.is_empty()
                && !avail.module.is_empty()
            {
                let version_opt = (!avail.version.is_empty()).then_some(avail.version.as_str());
                deps.push(DependencyCoordinate::new(
                    SmolStr::new(&avail.group),
                    SmolStr::new(&avail.module),
                    version_opt,
                ));
            }
            if let Some(ref dep_list) = variant.dependencies {
                for dependency in dep_list {
                    let ver_str = dependency.version.as_ref().and_then(|ver_constraint| {
                        ver_constraint
                            .requires
                            .as_deref()
                            .or(ver_constraint.strictly.as_deref())
                            .or(ver_constraint.prefers.as_deref())
                    });
                    let coord = DependencyCoordinate::new(
                        SmolStr::new(&dependency.group),
                        SmolStr::new(&dependency.module),
                        ver_str,
                    );
                    deps.push(coord);
                }
            }
        }

        deps
    }
}

fn variant_has_deps(variant: &ModuleVariant) -> bool {
    variant
        .dependencies
        .as_ref()
        .is_some_and(|list| !list.is_empty())
        || variant.available_at.is_some()
}

fn select_preferred_variant(variants: &[ModuleVariant]) -> Option<&ModuleVariant> {
    variants
        .iter()
        .find(|variant| {
            let lower = variant.name.to_ascii_lowercase();
            variant_has_deps(variant)
                && (lower.contains("jvmruntime")
                    || lower.contains("desktopruntime")
                    || lower.contains("androidruntime"))
        })
        .or_else(|| {
            variants.iter().find(|variant| {
                variant_has_deps(variant) && variant.name.to_ascii_lowercase().contains("runtime")
            })
        })
        .or_else(|| {
            variants.iter().find(|variant| {
                variant_has_deps(variant) && variant.name.to_ascii_lowercase().contains("api")
            })
        })
        .or_else(|| variants.iter().find(|variant| variant_has_deps(variant)))
        .or_else(|| variants.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_gradle_module_metadata() {
        let json = r#"{
            "formatVersion": "1.1",
            "component": {
                "group": "org.jetbrains.kotlinx",
                "module": "kotlinx-coroutines-core",
                "version": "1.8.0"
            },
            "variants": [
                {
                    "name": "jvmRuntimeElements",
                    "dependencies": [
                        {
                            "group": "org.jetbrains.kotlin",
                            "module": "kotlin-stdlib",
                            "version": { "requires": "1.9.21" }
                        }
                    ]
                }
            ]
        }"#;

        let meta = GradleModuleMetadata::parse(json).unwrap();
        assert_eq!(meta.format_version.as_deref(), Some("1.1"));
        let deps = meta.resolved_dependencies();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].group, "org.jetbrains.kotlin");
        assert_eq!(deps[0].name, "kotlin-stdlib");
        assert_eq!(deps[0].version.as_ref().unwrap().raw(), "1.9.21");
    }
}

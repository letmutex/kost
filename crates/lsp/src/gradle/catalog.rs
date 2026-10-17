use ahash::AHashMap;
use smol_str::SmolStr;
use toml::Value;

use crate::gradle::dependency::DependencyCoordinate;

/// Parsed Gradle Version Catalog (e.g. `gradle/libs.versions.toml`).
#[derive(Debug, Clone, Default)]
pub struct VersionCatalog {
    pub versions: AHashMap<String, String>,
    pub libraries: AHashMap<String, DependencyCoordinate>,
    pub bundles: AHashMap<String, Vec<String>>,
    pub plugins: AHashMap<String, (String, Option<String>)>,
}

impl VersionCatalog {
    pub fn parse(toml_str: &str) -> Result<Self, toml::de::Error> {
        let root: Value = toml::from_str(toml_str)?;
        let mut catalog = VersionCatalog::default();

        // Parse [versions]
        if let Some(versions_tbl) = root.get("versions").and_then(|v| v.as_table()) {
            for (k, v) in versions_tbl {
                let normalized_key = normalize_catalog_accessor(k);
                if let Some(s) = extract_version_string(v) {
                    catalog.versions.insert(k.clone(), s.clone());
                    catalog.versions.insert(normalized_key, s);
                }
            }
        }

        // Parse [libraries]
        if let Some(libs_tbl) = root.get("libraries").and_then(|v| v.as_table()) {
            for (k, v) in libs_tbl {
                let normalized_key = normalize_catalog_accessor(k);
                if let Some(dep) = parse_library_entry(v, &catalog.versions) {
                    catalog.libraries.insert(k.clone(), dep.clone());
                    catalog.libraries.insert(normalized_key, dep);
                }
            }
        }

        // Parse [bundles]
        if let Some(bundles_tbl) = root.get("bundles").and_then(|v| v.as_table()) {
            for (k, v) in bundles_tbl {
                if let Some(arr) = v.as_array() {
                    let items: Vec<String> = arr
                        .iter()
                        .filter_map(|item| item.as_str().map(|s| s.to_string()))
                        .collect();
                    catalog.bundles.insert(k.clone(), items.clone());
                    catalog.bundles.insert(normalize_catalog_accessor(k), items);
                }
            }
        }

        // Parse [plugins]
        if let Some(plugins_tbl) = root.get("plugins").and_then(|v| v.as_table()) {
            for (k, v) in plugins_tbl {
                if let Some(p) = parse_plugin_entry(v, &catalog.versions) {
                    catalog.plugins.insert(k.clone(), p.clone());
                    catalog.plugins.insert(normalize_catalog_accessor(k), p);
                }
            }
        }

        Ok(catalog)
    }

    /// Resolve a catalog accessor like `libs.androidx.core.ktx` or `androidx-core-ktx` or `libs.bundles.compose`
    pub fn resolve_accessor(&self, accessor: &str) -> Vec<DependencyCoordinate> {
        let clean = accessor
            .trim_start_matches("libs.")
            .trim_start_matches("libraries.");

        // Check bundle
        if let Some(bundle_name) = clean.strip_prefix("bundles.") {
            let norm_bundle = normalize_catalog_accessor(bundle_name);
            if let Some(lib_names) = self
                .bundles
                .get(bundle_name)
                .or_else(|| self.bundles.get(&norm_bundle))
            {
                return lib_names
                    .iter()
                    .filter_map(|name| self.resolve_single_library(name))
                    .collect();
            }
        }

        if let Some(dep) = self.resolve_single_library(clean) {
            vec![dep]
        } else {
            Vec::new()
        }
    }

    pub fn resolve_single_library(&self, name: &str) -> Option<DependencyCoordinate> {
        let norm = normalize_catalog_accessor(name);
        self.libraries
            .get(name)
            .or_else(|| self.libraries.get(&norm))
            .cloned()
    }

    /// Return all dependencies declared in the catalog
    pub fn all_dependencies(&self) -> Vec<DependencyCoordinate> {
        let mut unique: AHashMap<(SmolStr, SmolStr), DependencyCoordinate> = AHashMap::default();
        for dep in self.libraries.values() {
            unique.insert(dep.key(), dep.clone());
        }
        unique.into_values().collect()
    }
}

/// Normalize accessor: `androidx-core-ktx` -> `androidx.core.ktx` or `androidx_core_ktx`
pub fn normalize_catalog_accessor(s: &str) -> String {
    s.replace(['-', '_'], ".")
}

fn extract_version_string(val: &Value) -> Option<String> {
    match val {
        Value::String(s) => Some(s.clone()),
        Value::Table(t) => t
            .get("strictly")
            .or_else(|| t.get("prefer"))
            .or_else(|| t.get("require"))
            .or_else(|| t.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

fn parse_library_entry(
    val: &Value,
    versions: &AHashMap<String, String>,
) -> Option<DependencyCoordinate> {
    match val {
        Value::String(s) => DependencyCoordinate::parse(s),
        Value::Table(tbl) => {
            let mut group = None;
            let mut name = None;

            if let Some(module) = tbl.get("module").and_then(|v| v.as_str()) {
                let parts: Vec<&str> = module.split(':').collect();
                if parts.len() >= 2 {
                    group = Some(parts[0].to_string());
                    name = Some(parts[1].to_string());
                }
            }

            if group.is_none() {
                group = tbl
                    .get("group")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }
            if name.is_none() {
                name = tbl
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
            }

            let group = group?;
            let name = name?;

            let mut version_str = None;
            if let Some(ver_val) = tbl.get("version") {
                match ver_val {
                    Value::String(s) => version_str = Some(s.clone()),
                    Value::Table(vt) => {
                        if let Some(vref) = vt.get("ref").and_then(|r| r.as_str()) {
                            let norm_ref = normalize_catalog_accessor(vref);
                            version_str = versions
                                .get(vref)
                                .or_else(|| versions.get(&norm_ref))
                                .cloned();
                        } else {
                            version_str = extract_version_string(ver_val);
                        }
                    }
                    _ => {}
                }
            }

            Some(DependencyCoordinate::new(
                group,
                name,
                version_str.as_deref(),
            ))
        }
        _ => None,
    }
}

fn parse_plugin_entry(
    val: &Value,
    versions: &AHashMap<String, String>,
) -> Option<(String, Option<String>)> {
    match val {
        Value::String(s) => Some((s.clone(), None)),
        Value::Table(tbl) => {
            let id = tbl.get("id").and_then(|v| v.as_str())?.to_string();
            let mut version_str = None;
            if let Some(ver_val) = tbl.get("version") {
                match ver_val {
                    Value::String(s) => version_str = Some(s.clone()),
                    Value::Table(vt) => {
                        if let Some(vref) = vt.get("ref").and_then(|r| r.as_str()) {
                            let norm_ref = normalize_catalog_accessor(vref);
                            version_str = versions
                                .get(vref)
                                .or_else(|| versions.get(&norm_ref))
                                .cloned();
                        } else {
                            version_str = extract_version_string(ver_val);
                        }
                    }
                    _ => {}
                }
            }
            Some((id, version_str))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::VersionCatalog;

    #[test]
    fn test_parse_catalog_toml() {
        let toml_sample = r#"
[versions]
kotlin = "2.0.20"
coreKtx = "1.15.0"
compose = { strictly = "1.7.0" }

[libraries]
androidx-core-ktx = { group = "androidx.core", name = "core-ktx", version.ref = "coreKtx" }
androidx-appcompat = { module = "androidx.appcompat:appcompat", version = "1.7.0" }
gson = "com.google.code.gson:gson:2.10.1"

[bundles]
core = ["androidx-core-ktx", "androidx-appcompat"]

[plugins]
kotlin-android = { id = "org.jetbrains.kotlin.android", version.ref = "kotlin" }
"#;

        let catalog = VersionCatalog::parse(toml_sample).unwrap();
        assert_eq!(catalog.versions.get("kotlin").unwrap(), "2.0.20");

        let core_ktx = catalog.resolve_accessor("libs.androidx.core.ktx");
        assert_eq!(core_ktx.len(), 1);
        assert_eq!(core_ktx[0].group, "androidx.core");
        assert_eq!(core_ktx[0].name, "core-ktx");
        assert_eq!(core_ktx[0].version.as_ref().unwrap().raw(), "1.15.0");

        let bundle = catalog.resolve_accessor("libs.bundles.core");
        assert_eq!(bundle.len(), 2);
    }
}

use ahash::AHashMap;
use smol_str::SmolStr;
use toml::Value;

use crate::coordinate::DependencyCoordinate;

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

        if let Some(versions_tbl) = root.get("versions").and_then(Value::as_table) {
            for (key, val) in versions_tbl {
                if let Some(version_str) = extract_version_string(val) {
                    catalog.versions.insert(key.clone(), version_str.clone());
                    catalog
                        .versions
                        .insert(key.replace('-', "."), version_str.clone());
                    catalog
                        .versions
                        .insert(key.replace('-', "_"), version_str.clone());
                    catalog
                        .versions
                        .insert(normalize_catalog_accessor(key), version_str);
                }
            }
        }

        if let Some(libs_tbl) = root.get("libraries").and_then(Value::as_table) {
            for (key, val) in libs_tbl {
                let normalized_key = normalize_catalog_accessor(key);
                if let Some(dep) = parse_library_entry(val, &catalog.versions) {
                    catalog.libraries.insert(key.clone(), dep.clone());
                    catalog.libraries.insert(normalized_key, dep);
                }
            }
        }

        if let Some(bundles_tbl) = root.get("bundles").and_then(Value::as_table) {
            for (key, val) in bundles_tbl {
                if let Some(arr) = val.as_array() {
                    let items: Vec<String> = arr
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_string))
                        .collect();
                    catalog.bundles.insert(key.clone(), items.clone());
                    catalog
                        .bundles
                        .insert(normalize_catalog_accessor(key), items);
                }
            }
        }

        if let Some(plugins_tbl) = root.get("plugins").and_then(Value::as_table) {
            for (key, val) in plugins_tbl {
                if let Some(plugin) = parse_plugin_entry(val, &catalog.versions) {
                    catalog.plugins.insert(key.clone(), plugin.clone());
                    catalog
                        .plugins
                        .insert(normalize_catalog_accessor(key), plugin);
                }
            }
        }

        Ok(catalog)
    }

    /// Resolve a catalog accessor like `libs.androidx.core.ktx`, `androidx-core-ktx`, or `libs.bundles.compose`.
    pub fn resolve_accessor(&self, accessor: &str) -> Vec<DependencyCoordinate> {
        let clean = accessor
            .trim()
            .trim_start_matches("libs.")
            .trim_start_matches("libraries.");

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

    /// Resolve a plugin catalog accessor like `libs.plugins.kotlin.android`.
    pub fn resolve_plugin_accessor(&self, accessor: &str) -> Option<(String, Option<String>)> {
        let clean = accessor
            .trim()
            .trim_start_matches("libs.")
            .trim_start_matches("plugins.");
        let norm = normalize_catalog_accessor(clean);
        self.plugins
            .get(clean)
            .or_else(|| self.plugins.get(&norm))
            .cloned()
    }

    /// Return all unique dependencies declared in the catalog.
    pub fn all_dependencies(&self) -> Vec<DependencyCoordinate> {
        let mut unique: AHashMap<(SmolStr, SmolStr), DependencyCoordinate> = AHashMap::default();
        for dep in self.libraries.values() {
            unique.insert(dep.key(), dep.clone());
        }
        unique.into_values().collect()
    }
}

/// Parse all library entries from a version catalog TOML string into `deps`.
pub fn parse_catalog_dependencies(content: &str, deps: &mut Vec<DependencyCoordinate>) {
    if let Ok(catalog) = VersionCatalog::parse(content) {
        deps.extend(catalog.all_dependencies());
    }
}

/// Normalize accessor: `androidx-core-ktx` -> `androidx.core.ktx`.
pub fn normalize_catalog_accessor(accessor: &str) -> String {
    accessor.replace(['-', '_'], ".")
}

fn extract_version_string(value: &Value) -> Option<String> {
    match value {
        Value::String(version_str) => Some(version_str.clone()),
        Value::Table(table) => table
            .get("strictly")
            .or_else(|| table.get("prefer"))
            .or_else(|| table.get("require"))
            .or_else(|| table.get("version"))
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

fn parse_library_entry(
    value: &Value,
    versions: &AHashMap<String, String>,
) -> Option<DependencyCoordinate> {
    match value {
        Value::String(coord_str) => DependencyCoordinate::parse(coord_str),
        Value::Table(table) => {
            let (group, name, inline_ver) = resolve_library_group_name(table)?;
            let version = resolve_table_version(table, versions).or(inline_ver);
            Some(DependencyCoordinate::new(group, name, version.as_deref()))
        }
        _ => None,
    }
}

fn resolve_library_group_name(
    table: &toml::map::Map<String, Value>,
) -> Option<(String, String, Option<String>)> {
    if let Some(module) = table.get("module").and_then(Value::as_str) {
        let mut parts = module.split(':');
        let group = parts.next()?.trim();
        let name = parts.next()?.trim();
        if group.is_empty() || name.is_empty() {
            return None;
        }
        let inline_ver = parts
            .next()
            .map(str::trim)
            .filter(|ver| !ver.is_empty())
            .map(String::from);
        return Some((group.to_string(), name.to_string(), inline_ver));
    }
    let group = table.get("group").and_then(Value::as_str)?.trim();
    let name = table.get("name").and_then(Value::as_str)?.trim();
    if group.is_empty() || name.is_empty() {
        return None;
    }
    Some((group.to_string(), name.to_string(), None))
}

fn resolve_table_version(
    table: &toml::map::Map<String, Value>,
    versions: &AHashMap<String, String>,
) -> Option<String> {
    if let Some(ver_val) = table.get("version") {
        if let Some(version_str) = extract_version_string(ver_val) {
            return Some(version_str);
        }
        if let Some(ver_ref) = ver_val.get("ref").and_then(Value::as_str) {
            return lookup_version_ref(ver_ref, versions);
        }
    }
    let ver_ref = table.get("version.ref").and_then(Value::as_str)?;
    lookup_version_ref(ver_ref, versions)
}

fn lookup_version_ref(ver_ref: &str, versions: &AHashMap<String, String>) -> Option<String> {
    versions
        .get(ver_ref)
        .or_else(|| versions.get(&normalize_catalog_accessor(ver_ref)))
        .cloned()
}

fn parse_plugin_entry(
    value: &Value,
    versions: &AHashMap<String, String>,
) -> Option<(String, Option<String>)> {
    match value {
        Value::String(plugin_str) => {
            if let Some((id, ver)) = plugin_str.split_once(':') {
                Some((id.trim().to_string(), Some(ver.trim().to_string())))
            } else {
                Some((plugin_str.trim().to_string(), None))
            }
        }
        Value::Table(table) => {
            let id = table.get("id").and_then(Value::as_str)?.trim().to_string();
            let version_str = resolve_table_version(table, versions);
            Some((id, version_str))
        }
        _ => None,
    }
}

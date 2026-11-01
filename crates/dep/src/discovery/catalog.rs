use ahash::AHashMap;

use crate::coordinate::DependencyCoordinate;

pub(crate) fn parse_catalog_dependencies(content: &str, deps: &mut Vec<DependencyCoordinate>) {
    let Ok(root) = toml::from_str::<toml::Value>(content) else {
        return;
    };

    let mut versions = AHashMap::default();
    if let Some(version_table) = root.get("versions").and_then(|val| val.as_table()) {
        for (key, val) in version_table {
            if let Some(version_str) = extract_raw_version(val) {
                versions.insert(key.clone(), version_str.clone());
                versions.insert(key.replace('-', "."), version_str.clone());
                versions.insert(key.replace('-', "_"), version_str.clone());
                versions.insert(key.replace(['-', '_'], "."), version_str);
            }
        }
    }

    if let Some(libs_table) = root.get("libraries").and_then(|val| val.as_table()) {
        for entry_val in libs_table.values() {
            if let Some(coord) = parse_library_entry(entry_val, &versions) {
                deps.push(coord);
            }
        }
    }
}

fn extract_raw_version(value: &toml::Value) -> Option<String> {
    match value {
        toml::Value::String(version_str) => Some(version_str.clone()),
        toml::Value::Table(table) => table
            .get("strictly")
            .or_else(|| table.get("prefer"))
            .or_else(|| table.get("require"))
            .or_else(|| table.get("version"))
            .and_then(|val| val.as_str())
            .map(|version_str| version_str.to_string()),
        _ => None,
    }
}

fn parse_library_entry(
    value: &toml::Value,
    versions: &AHashMap<String, String>,
) -> Option<DependencyCoordinate> {
    match value {
        toml::Value::String(coord_str) => DependencyCoordinate::parse(coord_str),
        toml::Value::Table(table) => {
            let (group, name, inline_ver) = resolve_library_group_name(table)?;
            let version = resolve_library_version(table, versions).or(inline_ver);
            Some(DependencyCoordinate::new(group, name, version.as_deref()))
        }
        _ => None,
    }
}

fn resolve_library_group_name(
    table: &toml::map::Map<String, toml::Value>,
) -> Option<(String, String, Option<String>)> {
    if let Some(module) = table.get("module").and_then(|val| val.as_str()) {
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
    let group = table.get("group").and_then(|val| val.as_str())?.trim();
    let name = table.get("name").and_then(|val| val.as_str())?.trim();
    if group.is_empty() || name.is_empty() {
        return None;
    }
    Some((group.to_string(), name.to_string(), None))
}

fn resolve_library_version(
    table: &toml::map::Map<String, toml::Value>,
    versions: &AHashMap<String, String>,
) -> Option<String> {
    if let Some(ver_val) = table.get("version") {
        if let Some(version_str) = extract_raw_version(ver_val) {
            return Some(version_str);
        }
        if let Some(ver_ref) = ver_val.get("ref").and_then(|ref_val| ref_val.as_str()) {
            return versions
                .get(ver_ref)
                .or_else(|| versions.get(&ver_ref.replace(['-', '_'], ".")))
                .cloned();
        }
    }
    let ver_ref = table.get("version.ref").and_then(|val| val.as_str())?;
    versions
        .get(ver_ref)
        .or_else(|| versions.get(&ver_ref.replace(['-', '_'], ".")))
        .cloned()
}

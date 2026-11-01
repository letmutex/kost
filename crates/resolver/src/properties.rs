use std::path::{Path, PathBuf};

use ahash::AHashMap;

pub fn collect_gradle_properties(scripts: &[PathBuf]) -> AHashMap<String, String> {
    let mut properties = AHashMap::default();
    for script_path in scripts {
        if script_path.file_name().and_then(|name| name.to_str()) == Some("gradle.properties")
            && let Ok(content) = std::fs::read_to_string(script_path)
        {
            parse_gradle_properties(&content, &mut properties);
        }
    }
    properties
}

pub fn parse_gradle_properties(content: &str, props: &mut AHashMap<String, String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=').or_else(|| trimmed.split_once(':')) {
            props.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
}

pub(crate) fn inherit_ancestor_vars(
    script_path: &Path,
    dir_vars: &AHashMap<PathBuf, AHashMap<String, String>>,
) -> AHashMap<String, String> {
    let mut merged = AHashMap::default();
    let Some(script_dir) = script_path.parent() else {
        return merged;
    };
    let ancestors: Vec<&Path> = script_dir.ancestors().collect();
    for ancestor in ancestors.into_iter().rev() {
        if let Some(vars) = dir_vars.get(ancestor) {
            merged.extend(vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
    }
    merged
}

pub(crate) fn store_dir_vars(
    script_path: &Path,
    variables: AHashMap<String, String>,
    dir_vars: &mut AHashMap<PathBuf, AHashMap<String, String>>,
) {
    if let Some(script_dir) = script_path.parent() {
        dir_vars
            .entry(script_dir.to_path_buf())
            .or_default()
            .extend(variables);
    }
}

pub(crate) fn populate_gradle_properties_by_dir(
    scripts: &[PathBuf],
    dir_vars: &mut AHashMap<PathBuf, AHashMap<String, String>>,
) {
    for script_path in scripts {
        if script_path.file_name().and_then(|name| name.to_str()) == Some("gradle.properties")
            && let Ok(content) = std::fs::read_to_string(script_path)
        {
            let mut props = AHashMap::default();
            parse_gradle_properties(&content, &mut props);
            store_dir_vars(script_path, props, dir_vars);
        }
    }
}

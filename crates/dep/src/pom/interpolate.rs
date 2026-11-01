use ahash::AHashMap;
use smol_str::SmolStr;

use super::{PomDependency, PomModel};

/// Interpolates variable references `${prop}` using POM properties and inherited variables.
pub(crate) fn interpolate_pom(
    model: &mut PomModel,
    inherited_properties: &AHashMap<String, String>,
) {
    if model.group.is_none()
        && let Some(ref parent_info) = model.parent
    {
        model.group = Some(parent_info.group.clone());
    }
    if model.version.is_none()
        && let Some(ref parent_info) = model.parent
    {
        model.version = Some(parent_info.version.clone());
    }

    let merged = build_interpolation_properties(model, inherited_properties);

    for dep in &mut model.dependencies {
        interpolate_dep(dep, &merged);
    }
    for dep in &mut model.dependency_management {
        interpolate_dep(dep, &merged);
    }

    apply_dependency_management_versions(model);
}

fn build_interpolation_properties(
    model: &mut PomModel,
    inherited_properties: &AHashMap<String, String>,
) -> AHashMap<String, String> {
    let mut merged = inherited_properties.clone();
    merged.extend(model.properties.clone());

    if let Some(ref group_val) = model.group
        && group_val.contains("${")
    {
        model.group = Some(SmolStr::new(substitute_variables(group_val, &merged)));
    }
    if let Some(ref ver_val) = model.version
        && ver_val.contains("${")
    {
        model.version = Some(substitute_variables(ver_val, &merged));
    }

    insert_alias_triplet(&mut merged, "groupId", model.group.as_deref());
    insert_alias_triplet(&mut merged, "artifactId", model.name.as_deref());
    insert_alias_triplet(&mut merged, "version", model.version.as_deref());

    if let Some(ref parent_info) = model.parent {
        for (suffix, val) in [
            ("groupId", parent_info.group.as_str()),
            ("artifactId", parent_info.name.as_str()),
            ("version", parent_info.version.as_str()),
        ] {
            let proj_key = common::concat_str!("project.parent.", suffix);
            merged.insert(proj_key, val.to_string());
            merged.insert(common::concat_str!("parent.", suffix), val.to_string());
        }
    }
    merged
}

fn insert_alias_triplet(merged: &mut AHashMap<String, String>, key: &str, value: Option<&str>) {
    if let Some(val) = value {
        merged.insert(common::concat_str!("project.", key), val.to_string());
        merged.insert(common::concat_str!("pom.", key), val.to_string());
        merged
            .entry(key.to_string())
            .or_insert_with(|| val.to_string());
    }
}

fn apply_dependency_management_versions(model: &mut PomModel) {
    let mgmt_versions: AHashMap<(&str, &str), &str> = model
        .dependency_management
        .iter()
        .filter_map(|dep| {
            dep.version
                .as_deref()
                .map(|ver| ((dep.group.as_str(), dep.name.as_str()), ver))
        })
        .collect();

    for dep in &mut model.dependencies {
        if dep.version.is_none()
            && let Some(&ver) = mgmt_versions.get(&(dep.group.as_str(), dep.name.as_str()))
        {
            dep.version = Some(ver.to_string());
        }
    }
}

fn interpolate_dep(dep: &mut PomDependency, properties: &AHashMap<String, String>) {
    if dep.group.contains("${") {
        dep.group = SmolStr::new(substitute_variables(&dep.group, properties));
    }
    if dep.name.contains("${") {
        dep.name = SmolStr::new(substitute_variables(&dep.name, properties));
    }
    if let Some(ref ver) = dep.version {
        dep.version = Some(substitute_variables(ver, properties));
    }
}

pub(crate) fn substitute_variables(
    template: &str,
    properties: &AHashMap<String, String>,
) -> String {
    let mut result = template.to_string();
    for _ in 0..4 {
        if !result.contains("${") {
            break;
        }
        let mut changed = false;
        for (key, value) in properties {
            let needle = common::concat_str!("${", key, "}");
            if result.contains(&needle) {
                result = result.replace(&needle, value);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    result
}

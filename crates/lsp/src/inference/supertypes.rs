use crate::indexer::WorkspaceIndex;
use crate::inference::types::KotlinType;

pub fn builtin_progression_supertype(class_name: &str) -> Option<&'static str> {
    let simple = class_name.rsplit('.').next().unwrap_or(class_name);
    match simple {
        "IntRange" | "IntProgression" => Some("Iterable<Int>"),
        "LongRange" | "LongProgression" => Some("Iterable<Long>"),
        "CharRange" | "CharProgression" => Some("Iterable<Char>"),
        "UIntRange" | "UIntProgression" => Some("Iterable<UInt>"),
        "ULongRange" | "ULongProgression" => Some("Iterable<ULong>"),
        _ => None,
    }
}

/// Resolves generic supertypes for a given class name using the index.
pub fn get_generic_supertypes(index: &WorkspaceIndex, class_name: &str) -> Vec<KotlinType> {
    let simple = class_name.rsplit('.').next().unwrap_or(class_name);
    let supers = index
        .class_supertypes
        .get(class_name)
        .or_else(|| index.class_supertypes.get(simple));
    let mut result: Vec<KotlinType> = supers
        .into_iter()
        .flatten()
        .filter_map(|super_str| crate::inference::types::parse_type_signature(super_str))
        .collect();
    if let Some(builtin_sup) = builtin_progression_supertype(simple)
        && let Some(parsed) = crate::inference::types::parse_type_signature(builtin_sup)
        && !result.contains(&parsed)
    {
        result.push(parsed);
    }
    result
}

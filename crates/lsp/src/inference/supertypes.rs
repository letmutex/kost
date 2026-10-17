use crate::indexer::WorkspaceIndex;
use crate::inference::types::KotlinType;

/// Resolves generic supertypes for a given class name using the index.
pub fn get_generic_supertypes(index: &WorkspaceIndex, class_name: &str) -> Vec<KotlinType> {
    let simple = class_name.rsplit('.').next().unwrap_or(class_name);
    let supers = index
        .class_supertypes
        .get(class_name)
        .or_else(|| index.class_supertypes.get(simple));
    let Some(supers) = supers else {
        return Vec::new();
    };
    supers
        .iter()
        .filter_map(|s| crate::inference::types::parse_type_signature(s))
        .collect()
}

use tree_sitter::Node;

use super::BuildScriptInfo;
use super::groovy_android::extract_groovy_android_closure;
use super::groovy_plugins::extract_groovy_plugins_closure;
use super::groovy_prop::find_invocation_closure;
use super::util::{groovy_call_target, node_text};
use crate::catalog::VersionCatalog;

/// Extracts `android { compileSdk }` and `plugins { ... }` metadata from Groovy DSL AST.
pub(crate) fn extract_groovy_metadata(
    node: Node,
    source: &[u8],
    catalog: Option<&VersionCatalog>,
    info: &mut BuildScriptInfo,
) {
    if matches!(node.kind(), "command_chain" | "method_invocation")
        && let Some(target) = groovy_call_target(node)
    {
        let target_name = node_text(target, source).trim();
        if target_name == "android"
            && let Some(closure) = find_invocation_closure(node)
        {
            info.has_android_block = true;
            extract_groovy_android_closure(closure, source, info);
            return;
        }
        if target_name == "plugins"
            && let Some(closure) = find_invocation_closure(node)
        {
            extract_groovy_plugins_closure(closure, source, catalog, info);
            return;
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_groovy_metadata(child, source, catalog, info);
    }
}

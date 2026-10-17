pub mod ast_type;
pub mod display;
pub mod model;
pub mod standard;

pub use ast_type::ast_node_to_kotlin_type;
pub use model::{KotlinType, PrimitiveType, TypeArg, Variance};
pub use standard::{StandardInterface, is_lub_ignored_name};

use crate::indexer::WorkspaceIndex;

/// Check if a KotlinType represents a functional type or a SAM (Single Abstract Method) interface
pub fn is_functional_or_sam(ty: &KotlinType, index: &WorkspaceIndex) -> bool {
    let unwrapped = ty.unwrap_nullable_ref();
    if unwrapped.is_function() {
        return true;
    }
    match unwrapped {
        KotlinType::Class { name, .. } => {
            let base = name.as_str();
            let simple = base.rsplit('.').next().unwrap_or(base);
            if simple.starts_with("Function")
                || simple.starts_with("KFunction")
                || matches!(
                    simple,
                    "Runnable"
                        | "Callable"
                        | "Consumer"
                        | "BiConsumer"
                        | "Supplier"
                        | "Predicate"
                        | "BiPredicate"
                        | "Comparator"
                        | "Action"
                )
            {
                return true;
            }
            is_sam_interface(index, simple)
        }
        _ => false,
    }
}

fn is_sam_interface(index: &WorkspaceIndex, simple: &str) -> bool {
    let Some(decls) = index.name_to_decls.get(simple) else {
        return false;
    };
    for (_, d) in decls {
        if !matches!(d.kind, tower_lsp::lsp_types::SymbolKind::INTERFACE) {
            continue;
        }
        if d.modifiers.is_fun_interface {
            return true;
        }
        if d.doc_comment
            .as_deref()
            .is_some_and(|doc| doc.contains("@FunctionalInterface"))
        {
            return true;
        }
        if let Some(members) = index.container_to_decls.get(simple) {
            let is_single = members
                .iter()
                .filter(|(_, m)| {
                    matches!(
                        m.kind,
                        tower_lsp::lsp_types::SymbolKind::METHOD
                            | tower_lsp::lsp_types::SymbolKind::FUNCTION
                    )
                })
                .count()
                == 1;
            if is_single {
                return true;
            }
        }
    }
    false
}

/// Parse a Kotlin type signature string with known type parameters into a structured `KotlinType`
pub fn parse_type_signature_with_params(sig: &str, type_params: &[&str]) -> Option<KotlinType> {
    ast_type::parse_type_signature_with_params(sig, type_params)
}

/// Parse a Kotlin type signature string into a structured `KotlinType`
pub fn parse_type_signature(sig: &str) -> Option<KotlinType> {
    parse_type_signature_with_params(sig, &[])
}

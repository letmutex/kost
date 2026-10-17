use crate::indexer::WorkspaceIndex;
use crate::inference::signature::{GenericParamInfo, SignatureInfo};
use crate::syntax::symbol_item::DeclarationItem;

/// Ephemeral, stack-scoped candidate wrapper eliminating quadratic signature re-parsing
#[derive(Debug, Clone)]
pub struct TypedCandidate<'a> {
    pub decl: &'a DeclarationItem,
    pub sig_info: SignatureInfo,
}

impl<'a> TypedCandidate<'a> {
    pub fn new(decl: &'a DeclarationItem) -> Self {
        let sig_info = SignatureInfo::from_decl(decl).unwrap_or_else(|| {
            let sig = decl.signature.as_deref().unwrap_or("");
            SignatureInfo::parse(sig).unwrap_or(SignatureInfo {
                type_params: Vec::new(),
                receiver: None,
                params: Vec::new(),
                return_type: None,
            })
        });
        Self { decl, sig_info }
    }
}

pub(crate) fn generic_permits_nullable_typed(
    simple_name: &str,
    type_params: &[GenericParamInfo],
    index: &WorkspaceIndex,
) -> bool {
    if let Some(p) = type_params.iter().find(|p| p.name == simple_name) {
        p.bounds.is_empty() || p.bounds.iter().all(|b| b.is_nullable())
    } else {
        simple_name.len() == 1
            && simple_name.starts_with(|c: char| c.is_ascii_uppercase())
            && !index.name_to_decls.contains_key(simple_name)
    }
}

pub(crate) fn is_unconstrained_generic_typed(
    simple_name: &str,
    type_params: &[GenericParamInfo],
    index: &WorkspaceIndex,
) -> bool {
    type_params
        .iter()
        .find(|p| p.name == simple_name)
        .is_some_and(|p| p.bounds.is_empty())
        || (simple_name.len() == 1
            && simple_name.starts_with(|c: char| c.is_ascii_uppercase())
            && !type_params.iter().any(|p| p.name == simple_name)
            && !index.name_to_decls.contains_key(simple_name))
}

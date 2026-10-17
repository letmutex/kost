use std::cmp::Reverse;
use std::collections::HashSet;

use crate::indexer::WorkspaceIndex;
use crate::inference::signature::{GenericParamInfo, SignatureInfo, SignatureParam};
use crate::inference::types::KotlinType;
use crate::syntax::ast_call_view::{ParsedCallArg, parse_call_arg};
use crate::syntax::symbol_item::DeclarationItem;

pub mod bounds;
pub mod call_args;
pub mod candidate;
pub mod primitive;
pub mod score_typed;

pub use bounds::{clean_variance, is_subtype, satisfies_bound};
pub use call_args::{
    extract_call_args_and_inferred_types, extract_call_args_and_inferred_types_opts,
    extract_call_site_arguments, extract_call_site_arguments_opts,
};
pub use candidate::TypedCandidate;
pub use score_typed::{
    compute_type_matches, is_primitive_incompatible, score_typed_candidate, type_match_score,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OverloadScore {
    pub type_matches: usize,
    pub named_matches: usize,
    pub exact_arity: bool,
    pub param_count_diff_penalty: Reverse<usize>,
    pub is_source: bool,
}

/// Score a candidate declaration against call arguments and inferred argument types
pub fn score_overload_candidate(
    decl: &DeclarationItem,
    is_src: bool,
    raw_args: &[&str],
    inferred_types: &[Option<KotlinType>],
    index: &WorkspaceIndex,
) -> Option<OverloadScore> {
    let call_args: Vec<ParsedCallArg> = raw_args.iter().map(|a| parse_call_arg(a)).collect();
    score_overload_candidate_parsed(decl, is_src, &call_args, inferred_types, index)
}

pub(crate) fn collect_container_type_params(
    container: &str,
    index: &WorkspaceIndex,
    type_params: &mut Vec<GenericParamInfo>,
) {
    let simple_container = container.rsplit('.').next().unwrap_or(container);
    if let Some(decls) = index.name_to_decls.get(simple_container) {
        let matching_classes: Vec<_> = decls
            .iter()
            .map(|(_, cd)| cd)
            .filter(|cd| {
                matches!(
                    cd.kind,
                    tower_lsp::lsp_types::SymbolKind::CLASS
                        | tower_lsp::lsp_types::SymbolKind::INTERFACE
                )
            })
            .collect();

        let candidates = if container.contains('.') {
            let qualified_match: Vec<_> = matching_classes
                .iter()
                .filter(|cd| {
                    cd.container_name
                        .as_deref()
                        .map(|cn| {
                            crate::concat_str!(cn, ".", cd.name) == container || cn == container
                        })
                        .unwrap_or(false)
                })
                .copied()
                .collect();
            if !qualified_match.is_empty() {
                qualified_match
            } else {
                matching_classes
            }
        } else {
            matching_classes
        };

        for class_decl in candidates {
            if let Some(sig_info) = SignatureInfo::from_decl(class_decl) {
                for cp in &sig_info.type_params {
                    if !type_params.iter().any(|p| p.name == cp.name) {
                        type_params.push(cp.clone());
                    }
                }
            }
        }
    }
}

/// Score a candidate declaration against pre-parsed call arguments
pub fn score_overload_candidate_parsed(
    decl: &DeclarationItem,
    is_src: bool,
    call_args: &[ParsedCallArg],
    inferred_types: &[Option<KotlinType>],
    index: &WorkspaceIndex,
) -> Option<OverloadScore> {
    let candidate = TypedCandidate::new(decl);
    score_typed_candidate(&candidate, is_src, call_args, inferred_types, index)
}

pub(crate) fn map_call_args_to_params<'a>(
    params: &'a [SignatureParam],
    call_args: &[ParsedCallArg],
) -> Vec<Option<&'a SignatureParam>> {
    let mut assigned = vec![false; params.len()];
    let mut result = vec![None; call_args.len()];

    for (arg_idx, arg) in call_args.iter().enumerate() {
        if let Some(ref name) = arg.name
            && let Some(param_idx) = params.iter().position(|p| p.name == *name)
            && !assigned[param_idx]
        {
            assigned[param_idx] = true;
            result[arg_idx] = Some(&params[param_idx]);
        }
    }

    let mut next_unassigned = 0;
    for (arg_idx, arg) in call_args.iter().enumerate() {
        if arg.name.is_none() {
            while next_unassigned < params.len() && assigned[next_unassigned] {
                next_unassigned += 1;
            }
            if next_unassigned < params.len() {
                assigned[next_unassigned] = true;
                result[arg_idx] = Some(&params[next_unassigned]);
                next_unassigned += 1;
            } else if let Some(vararg_param) = params.iter().find(|p| p.is_vararg) {
                result[arg_idx] = Some(vararg_param);
            }
        }
    }

    result
}

pub(crate) fn validate_call_arguments(
    params: &[SignatureParam],
    call_args: &[ParsedCallArg],
) -> Option<usize> {
    let has_vararg = params.iter().any(|p| p.is_vararg);
    let positional_count = call_args.iter().filter(|a| a.name.is_none()).count();

    if !has_vararg && (positional_count > params.len() || call_args.len() > params.len()) {
        return None;
    }

    let mut seen_names = HashSet::new();
    let mut named_matches = 0;
    for arg in call_args {
        if let Some(ref name_str) = arg.name {
            if !params.iter().any(|p| p.name == *name_str) || !seen_names.insert(name_str.as_str())
            {
                return None;
            }
            named_matches += 1;
        }
    }

    Some(named_matches)
}

use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::call::CallInfo;
use super::overload::score_overload_candidate_parsed;
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex};
use crate::syntax::ast_call_view::{ParsedCallArg, parse_call_arg};

pub fn collect_candidate_containers(container_type: &str, index: &WorkspaceIndex) -> Vec<String> {
    let unnull = container_type.trim_end_matches('?').trim();
    let bare = unnull.split('<').next().unwrap_or(unnull).trim();
    let mut containers = vec![unnull.to_string()];
    if bare != unnull {
        containers.push(bare.to_string());
    }
    containers.extend(index.get_all_supertypes(bare));
    containers
}

pub fn format_selected_member(decl: &DeclarationItem, detail: &str, origin: &str) -> String {
    let sig_info = decl
        .signature
        .as_deref()
        .map(|s| common::concat_str!(" (`", s, "`)"))
        .unwrap_or_default();
    common::concat_str!(
        "Selected container member `",
        decl.name.as_str(),
        "`",
        sig_info,
        " ",
        detail,
        " ",
        origin
    )
}

pub fn format_candidate_score(
    decl: &DeclarationItem,
    is_src: bool,
    parsed_args: &[ParsedCallArg],
    inferred_types: &[Option<KotlinType>],
    index: &WorkspaceIndex,
) -> String {
    match score_overload_candidate_parsed(decl, is_src, parsed_args, inferred_types, index) {
        Some(s) if s.named_matches > 0 => {
            common::concat_str!(
                "score: match=",
                s.type_matches.to_string(),
                ", named=",
                s.named_matches.to_string()
            )
        }
        Some(s) => common::concat_str!("score: match=", s.type_matches.to_string()),
        None => "score: none".to_string(),
    }
}

pub fn record_ranked_candidates(
    tracer: &mut super::trace::Tracer,
    matching: &[(&Url, &DeclarationItem)],
    call_info: &CallInfo,
    inferred_arg_types: &[Option<KotlinType>],
    index: &WorkspaceIndex,
) {
    if !tracer.is_active() || matching.is_empty() {
        return;
    }
    let parsed_args: Vec<_> = call_info
        .value_args
        .iter()
        .map(|a| parse_call_arg(a))
        .collect();
    for (url, decl) in matching.iter().take(5) {
        let is_src = crate::indexer::is_source_url(url);
        let score_str =
            format_candidate_score(decl, is_src, &parsed_args, inferred_arg_types, index);
        let sig = decl.signature.as_deref().unwrap_or(decl.name.as_str());
        let origin = super::trace::format_origin(url);
        tracer.record_step(|| {
            common::concat_str!("Candidate [", score_str, "]: `", sig, "` ", origin)
        });
    }
    if matching.len() > 5 {
        tracer.record_step(|| {
            common::concat_str!(
                "... and ",
                (matching.len() - 5).to_string(),
                " more candidate(s)"
            )
        });
    }
}

pub fn sort_by_overload_score<'a>(
    matching: &mut [(&'a Url, &'a DeclarationItem)],
    call_info: &CallInfo,
    inferred_types: &[Option<KotlinType>],
    index: &WorkspaceIndex,
) {
    let parsed_args: Vec<_> = call_info
        .value_args
        .iter()
        .map(|a| parse_call_arg(a))
        .collect();
    matching.sort_by_cached_key(|(url, decl)| {
        let is_src = crate::indexer::is_source_url(url);
        std::cmp::Reverse(score_overload_candidate_parsed(
            decl,
            is_src,
            &parsed_args,
            inferred_types,
            index,
        ))
    });
}

pub fn infer_call_argument_types(
    index: &WorkspaceIndex,
    current_url: &Url,
    call_info: &CallInfo,
    pos: Position,
    source: &str,
    root: Option<Node>,
    depth: usize,
) -> Vec<Option<KotlinType>> {
    call_info
        .value_args
        .iter()
        .map(|arg_expr| {
            let parsed = parse_call_arg(arg_expr);
            super::expression::infer_receiver_type_depth(
                index,
                current_url,
                &parsed.expr,
                pos,
                source,
                root,
                depth + 1,
            )
        })
        .collect()
}

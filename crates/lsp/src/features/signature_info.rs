use std::sync::Arc;
use tower_lsp::lsp_types::{
    Documentation, MarkupContent, MarkupKind, Position, SignatureInformation, SymbolKind, Url,
};
use tree_sitter::Node;

use super::signature_parser::{CallContext, parse_signature_parameters};
use crate::indexer::{DeclarationItem, WorkspaceIndex};
use crate::inference::KotlinType;
use crate::inference::overload::{OverloadScore, score_overload_candidate_parsed};
use crate::inference::signature::parse_signature_params;

pub fn is_callable_kind(kind: SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR | SymbolKind::CLASS
    )
}

pub fn strip_function_body(sig: &str) -> &str {
    let mut paren_depth: i32 = 0;
    let mut angle_depth: i32 = 0;
    let mut found_close = false;
    let bytes = sig.as_bytes();
    for (i, c) in sig.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => {
                paren_depth -= 1;
                if paren_depth == 0 && angle_depth == 0 {
                    found_close = true;
                }
            }
            '<' => angle_depth += 1,
            '>' if i == 0 || bytes[i - 1] != b'-' => {
                angle_depth = (angle_depth - 1).max(0);
            }
            '=' if paren_depth == 0 && angle_depth == 0 && found_close => {
                return sig[..i].trim();
            }
            _ => {}
        }
    }
    sig
}

pub fn build_signature_information(
    decl: &DeclarationItem,
    active_param: u32,
) -> Option<SignatureInformation> {
    let label = if let Some(ref sig) = decl.signature {
        strip_function_body(sig).to_string()
    } else if decl.kind == SymbolKind::CLASS {
        format!("class {}()", decl.name)
    } else {
        format!("fun {}()", decl.name)
    };

    let doc_str = decl.doc_comment.as_ref().map(|d| d.as_str());
    let params = parse_signature_parameters(&label, doc_str);

    let documentation = doc_str.map(|cleaned| {
        Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: cleaned.to_string(),
        })
    });

    Some(SignatureInformation {
        label,
        documentation,
        parameters: Some(params),
        active_parameter: Some(active_param),
    })
}

pub fn select_active_signature(signatures: &[SignatureInformation], active_param: u32) -> u32 {
    let mut vararg_match = None;
    for (idx, sig) in signatures.iter().enumerate() {
        if let Some(ref params) = sig.parameters {
            if (active_param as usize) < params.len() {
                return idx as u32;
            }
            let has_vararg = sig.label.contains("vararg ")
                || params.iter().any(|p| match &p.label {
                    tower_lsp::lsp_types::ParameterLabel::Simple(s) => s.contains("vararg"),
                    _ => false,
                });
            if has_vararg && vararg_match.is_none() {
                vararg_match = Some(idx as u32);
            }
        }
    }
    vararg_match.unwrap_or(0)
}

pub struct SignatureMatchContext<'a> {
    pub index: &'a WorkspaceIndex,
    pub current_url: &'a Url,
    pub pos: Position,
    pub source: &'a str,
    pub root: Option<Node<'a>>,
}

pub fn select_best_signature_and_param(
    candidates: &[(Arc<DeclarationItem>, bool)],
    signatures: &[SignatureInformation],
    call_ctx: &CallContext,
    ctx: &SignatureMatchContext,
) -> (u32, u32) {
    let inferred_types: Vec<Option<KotlinType>> = call_ctx
        .call_args
        .iter()
        .map(|arg| {
            if arg.expr.trim().is_empty() {
                None
            } else {
                crate::inference::infer_receiver_type_depth(
                    ctx.index,
                    ctx.current_url,
                    &arg.expr,
                    ctx.pos,
                    ctx.source,
                    ctx.root,
                    1,
                )
            }
        })
        .collect();

    let mut best: Option<(usize, OverloadScore)> = None;
    if !call_ctx.call_args.is_empty() {
        for (idx, (decl, is_src)) in candidates.iter().enumerate() {
            let sig_str = decl.signature.as_deref().unwrap_or("");
            let params = parse_signature_params(sig_str);
            let has_vararg = params.iter().any(|p| p.is_vararg);
            if !has_vararg && (call_ctx.active_parameter as usize) >= params.len() {
                continue;
            }

            if let Some(score) = score_overload_candidate_parsed(
                decl,
                *is_src,
                &call_ctx.call_args,
                &inferred_types,
                ctx.index,
            ) && best.as_ref().is_none_or(|(_, s)| score > *s)
            {
                best = Some((idx, score));
            }
        }
    }

    let active_sig_idx = if let Some((best_idx, _)) = best {
        best_idx as u32
    } else {
        select_active_signature(signatures, call_ctx.active_parameter)
    };

    let resolved_param = if let Some(ref name) = call_ctx.active_arg_name {
        if let Some((decl, _)) = candidates.get(active_sig_idx as usize) {
            let sig_str = decl.signature.as_deref().unwrap_or("");
            let params = parse_signature_params(sig_str);
            params
                .iter()
                .position(|p| p.name == *name)
                .map(|p| p as u32)
                .unwrap_or(call_ctx.active_parameter)
        } else {
            call_ctx.active_parameter
        }
    } else {
        call_ctx.active_parameter
    };

    (active_sig_idx, resolved_param)
}

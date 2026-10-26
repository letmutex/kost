use tower_lsp::lsp_types::{SymbolKind, Url};

use super::call::{CallInfo, solve_call_generic_return_type};
use super::context::InferenceContext;
use super::expression::resolve_full_type_name;
use super::generics::extract_generic_type_params;
use super::qualification::is_declaration_visible;
use super::types::KotlinType;
use crate::indexer::{DeclarationItem, WorkspaceIndex};

fn is_matching_decl(
    index: &WorkspaceIndex,
    current_url: &Url,
    url: &Url,
    decl: &DeclarationItem,
    full: &str,
) -> bool {
    if !is_declaration_visible(index, current_url, url, decl) {
        return false;
    }
    if !full.contains('.') {
        return true;
    }
    let file = match index.files.get(url) {
        Some(f) => f,
        None => return false,
    };
    let pkg = file.package_name.as_deref();
    let container = decl.container_name.as_deref();
    match (pkg, container) {
        (Some(p), Some(c)) => full
            .strip_prefix(p)
            .and_then(|r| r.strip_prefix('.'))
            .and_then(|r| r.strip_prefix(c))
            .and_then(|r| r.strip_prefix('.'))
            .is_some_and(|r| r == decl.name.as_str()),
        (Some(p), None) => full
            .strip_prefix(p)
            .and_then(|r| r.strip_prefix('.'))
            .is_some_and(|r| r == decl.name.as_str()),
        (None, Some(c)) => full
            .strip_prefix(c)
            .and_then(|r| r.strip_prefix('.'))
            .is_some_and(|r| r == decl.name.as_str()),
        (None, None) => full == decl.name.as_str(),
    }
}

fn find_constructor_decl_name<'a>(
    index: &'a WorkspaceIndex,
    fn_name: &'a str,
    simple_target: &'a str,
) -> Option<&'a str> {
    if index.name_to_decls.contains_key(fn_name) {
        Some(fn_name)
    } else if index.name_to_decls.contains_key(simple_target) {
        Some(simple_target)
    } else {
        None
    }
}

fn resolve_constructor_full_name(
    index: &WorkspaceIndex,
    current_url: &Url,
    fn_name: &str,
    decls: &[(std::sync::Arc<Url>, std::sync::Arc<DeclarationItem>)],
) -> String {
    let mut full = resolve_full_type_name(index, current_url, fn_name);
    if full.contains("->")
        && let Some((u, d)) = decls.iter().find(|(u, d)| {
            matches!(d.kind, SymbolKind::CLASS | SymbolKind::CONSTRUCTOR)
                && is_declaration_visible(index, current_url, u, d)
        })
    {
        let pkg = index
            .files
            .get(u.as_ref())
            .and_then(|f| f.package_name.as_deref());
        full = match pkg {
            Some(p) => common::concat_str!(p, ".", d.name),
            None => d.name.to_string(),
        };
    }
    full
}

pub fn find_constructor_return_type(
    ctx: InferenceContext<'_, '_, '_>,
    fn_name: &str,
    call_info: &CallInfo,
) -> Option<KotlinType> {
    let raw_name =
        super::typealias::resolve_type_alias_or_import(ctx.index, ctx.current_url, fn_name);
    let effective_name = if raw_name.is_empty() {
        fn_name
    } else {
        &raw_name
    };
    let simple_target_name = effective_name.rsplit('.').next().unwrap_or(effective_name);

    if !fn_name.chars().next().is_some_and(|c| c.is_uppercase())
        && !simple_target_name
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase())
    {
        return None;
    }

    let decl_name = find_constructor_decl_name(ctx.index, fn_name, simple_target_name)?;
    let decls = ctx.index.name_to_decls.get(decl_name)?;
    let full = resolve_constructor_full_name(ctx.index, ctx.current_url, fn_name, decls);

    let mut sorted_decls: Vec<_> = decls
        .iter()
        .filter(|(url, decl)| {
            is_matching_decl(ctx.index, ctx.current_url, url.as_ref(), decl, &full)
        })
        .collect();

    let has_visible_class = sorted_decls
        .iter()
        .any(|(_, d)| matches!(d.kind, SymbolKind::CLASS | SymbolKind::CONSTRUCTOR));
    if !has_visible_class {
        return None;
    }

    if !call_info.explicit_type_args.is_empty() {
        let parameterized =
            common::concat_str!(full, "<", call_info.explicit_type_args.join(", "), ">");
        return KotlinType::parse(&parameterized);
    }

    let current_url = ctx.current_url;
    sorted_decls.sort_by_key(|(u, _)| if u.as_ref() == current_url { 0 } else { 1 });
    if let Some(solved) =
        infer_generic_constructor_return_type(ctx, &sorted_decls, &full, call_info)
    {
        return Some(solved);
    }

    KotlinType::parse(&full)
}

fn infer_generic_constructor_return_type(
    mut ctx: InferenceContext<'_, '_, '_>,
    sorted_decls: &[&(std::sync::Arc<Url>, std::sync::Arc<DeclarationItem>)],
    full: &str,
    call_info: &CallInfo,
) -> Option<KotlinType> {
    for (_url, decl) in sorted_decls {
        if matches!(decl.kind, SymbolKind::CLASS | SymbolKind::CONSTRUCTOR) {
            if let Some(sig_info) = super::signature::SignatureInfo::from_decl(decl) {
                if !sig_info.type_params.is_empty() && !call_info.value_args.is_empty() {
                    let template_type = KotlinType::Class {
                        name: smol_str::SmolStr::new(full),
                        args: sig_info
                            .type_params
                            .iter()
                            .map(|p| {
                                super::types::TypeArg::invariant(KotlinType::TypeParam(
                                    p.name.as_str().into(),
                                ))
                            })
                            .collect(),
                    };
                    let solved = super::call::solve_call_decl_return_type_traced(
                        ctx.sub(),
                        decl,
                        Some(&template_type),
                        None,
                        call_info,
                        None,
                    );
                    return Some(solved);
                }
            } else if let Some(sig) = decl.signature.as_deref() {
                let type_params = extract_generic_type_params(sig);
                if !type_params.is_empty() && !call_info.value_args.is_empty() {
                    let template = common::concat_str!(full, "<", type_params.join(", "), ">");
                    let solved =
                        solve_call_generic_return_type(ctx.sub(), sig, &template, None, call_info);
                    return Some(solved);
                }
            }
        }
    }
    None
}

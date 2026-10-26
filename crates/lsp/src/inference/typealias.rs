use ahash::AHashSet;
use tower_lsp::lsp_types::SymbolKind;
use url::Url;

use crate::indexer::WorkspaceIndex;
use crate::syntax::symbol_item::DeclarationItem;

pub fn is_type_alias_private(declarations: &[DeclarationItem], alias_name: &str) -> bool {
    declarations.iter().any(|d| {
        d.name.as_str() == alias_name && d.kind == SymbolKind::TYPE_PARAMETER && d.is_private
    })
}

fn split_type_and_args(type_name: &str) -> (String, Vec<String>, bool) {
    let clean = type_name.trim();
    let is_nullable = clean.ends_with('?');
    let unnull = clean.trim_end_matches('?').trim();
    let (base, args) = match unnull.split_once('<') {
        Some((b, _)) if !b.contains('(') => (
            b.trim().to_string(),
            crate::syntax::signature::extract_type_arguments_list(unnull),
        ),
        _ => (unnull.to_string(), Vec::new()),
    };
    (base, args, is_nullable)
}

fn expand_alias_target(
    index: &WorkspaceIndex,
    url: &Url,
    alias_name: &str,
    target: &str,
    type_args: &[String],
) -> String {
    if type_args.is_empty() {
        return target.to_string();
    }
    let type_params = index
        .files
        .get(url)
        .and_then(|f| {
            f.declarations.iter().find_map(|d| {
                if d.name.as_str() == alias_name && d.kind == SymbolKind::TYPE_PARAMETER {
                    d.signature
                        .as_deref()
                        .map(crate::inference::generics::extract_generic_type_params)
                } else {
                    None
                }
            })
        })
        .unwrap_or_default();

    if !type_params.is_empty() {
        let tp_refs: Vec<&str> = type_params.iter().map(|s| s.as_str()).collect();
        if let Some(target_ty) =
            crate::inference::types::parse_type_signature_with_params(target, &tp_refs)
        {
            let mut bindings = ahash::AHashMap::default();
            for (i, p) in type_params.iter().enumerate() {
                if let Some(arg) = type_args.get(i)
                    && let Some(arg_ty) = crate::inference::types::parse_type_signature(arg)
                {
                    bindings.insert(smol_str::SmolStr::new(p), arg_ty);
                }
            }
            return target_ty.substitute(&bindings).to_string();
        }
    }
    common::concat_str!(target, "<", type_args.join(", "), ">")
}

fn resolve_file_alias_or_import(
    index: &WorkspaceIndex,
    file: &crate::syntax::symbol_item::FileSymbols,
    file_url: &Url,
    base: &str,
    args: &[String],
) -> Option<String> {
    if let Some(target) = file.type_aliases.get(base)
        && target != base
    {
        return Some(expand_alias_target(index, file_url, base, target, args));
    }
    for import in &file.imports {
        if import.alias.as_deref() == Some(base) {
            let imported = import.path.rsplit('.').next().unwrap_or(base);
            return Some(if args.is_empty() {
                imported.to_string()
            } else {
                common::concat_str!(imported, "<", args.join(", "), ">")
            });
        }
    }
    None
}

fn resolve_cross_file_alias<'a>(
    index: &'a WorkspaceIndex,
    active_url: &Url,
    base: &str,
    args: &[String],
) -> Option<(&'a Url, String)> {
    let decls = index.name_to_decls.get(base)?;
    for (decl_url, decl) in decls {
        if decl.kind == SymbolKind::TYPE_PARAMETER
            && !decl.is_private
            && crate::inference::qualification::is_declaration_visible(
                index,
                active_url,
                decl_url.as_ref(),
                decl,
            )
            && let Some(target) = index
                .files
                .get(decl_url.as_ref())
                .and_then(|f| f.type_aliases.get(base))
        {
            let expanded = expand_alias_target(index, decl_url.as_ref(), base, target, args);
            return Some((decl_url.as_ref(), expanded));
        }
    }
    None
}

pub fn resolve_type_alias_or_import(
    index: &WorkspaceIndex,
    current_url: &Url,
    type_name: &str,
) -> String {
    let (mut current_base, mut current_args, is_nullable) = split_type_and_args(type_name);
    let mut active_url = current_url;
    let mut visited = AHashSet::new();

    while visited.insert(current_base.clone()) && visited.len() <= 5 {
        if let Some(file) = index.files.get(active_url)
            && let Some(expanded) =
                resolve_file_alias_or_import(index, file, active_url, &current_base, &current_args)
        {
            let (next_base, next_args, _) = split_type_and_args(&expanded);
            if next_base != current_base || next_args != current_args {
                current_base = next_base;
                current_args = next_args;
                continue;
            }
        }

        if let Some((next_url, target)) =
            resolve_cross_file_alias(index, active_url, &current_base, &current_args)
        {
            let (next_base, next_args, _) = split_type_and_args(&target);
            if next_base == current_base && next_args == current_args {
                break;
            }
            current_base = next_base;
            current_args = next_args;
            active_url = next_url;
        } else {
            break;
        }
    }

    let final_name = if current_args.is_empty() {
        current_base
    } else {
        common::concat_str!(current_base, "<", current_args.join(", "), ">")
    };

    if is_nullable && !final_name.ends_with('?') {
        common::concat_str!(final_name, "?")
    } else {
        final_name
    }
}

use tower_lsp::lsp_types::SymbolKind;

use super::types::standard::StandardInterface;

/// Extract declared generic type parameters from a function or class signature (e.g. `<T, R>`)
pub fn extract_generic_type_params(sig: &str) -> Vec<String> {
    crate::syntax::signature::parse_signature_type_params(sig)
        .into_iter()
        .map(|p| p.name.to_string())
        .collect()
}

/// Split comma-separated type arguments respecting nested angle brackets and parentheses
pub fn split_type_arguments(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    let bytes = s.as_bytes();
    for (i, c) in s.char_indices() {
        if in_string {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => in_string = true,
            '<' | '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            '>' if i == 0 || bytes[i - 1] != b'-' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }
    parts
}

/// Extract type arguments from a generic type string (e.g. `Map<String, User>` -> `["String", "User"]`)
pub fn extract_type_arguments_list(type_str: &str) -> Vec<String> {
    let start_idx = match type_str.find('<') {
        Some(s) => s,
        None => return Vec::new(),
    };
    let bytes = type_str.as_bytes();
    let mut depth = 0;
    let mut end_idx = None;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in type_str[start_idx..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '<' => depth += 1,
            '>' if depth > 0 && (i == 0 || bytes[start_idx + i - 1] != b'-') => {
                depth -= 1;
                if depth == 0 {
                    end_idx = Some(start_idx + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let Some(end) = end_idx else {
        return Vec::new();
    };
    split_type_arguments(&type_str[start_idx + 1..end])
}

pub use crate::syntax::signature::extract_functional_return_type;

/// Solve and substitute generic return type based on receiver and lambda return types
pub fn solve_generic_return_type(
    sig: &str,
    decl_ret_type: &str,
    receiver_type_str: Option<&str>,
    lambda_return_type_str: Option<&str>,
    arg_idx: usize,
) -> String {
    let dummy_index = crate::indexer::WorkspaceIndex::new();
    let mut system = super::solver::ConstraintSystem::from_signature(&dummy_index, sig);

    if let Some(rec_str) = receiver_type_str {
        system.add_receiver_constraint(sig, rec_str);
    }
    if let Some(lambda_ret) = lambda_return_type_str {
        system.add_lambda_return_constraint(sig, lambda_ret, arg_idx);
    }

    let target = system
        .parse_with_params(decl_ret_type)
        .or_else(|| super::types::parse_type_signature(decl_ret_type));
    if let Some(t) = target {
        system.solve_and_substitute(&t).to_string()
    } else {
        decl_ret_type.to_string()
    }
}

/// Check whether an actual receiver type is compatible with a declaration receiver template
pub fn is_receiver_compatible_typed(
    index: &crate::indexer::WorkspaceIndex,
    actual_receiver: &super::types::KotlinType,
    decl_receiver: &super::types::KotlinType,
    type_params: &[smol_str::SmolStr],
) -> bool {
    let mut system = super::solver::ConstraintSystem::new(index, type_params.to_vec());
    system.add_subtype_constraint(actual_receiver, decl_receiver)
}

pub fn is_receiver_compatible(
    index: &crate::indexer::WorkspaceIndex,
    actual_receiver: &str,
    decl_receiver: &str,
    type_params: &[String],
) -> bool {
    let tp_smol: Vec<smol_str::SmolStr> = type_params.iter().map(smol_str::SmolStr::new).collect();
    let tp_refs: Vec<&str> = type_params.iter().map(|s| s.as_str()).collect();
    if let (Some(actual), Some(expected)) = (
        super::types::parse_type_signature(actual_receiver),
        super::types::parse_type_signature_with_params(decl_receiver, &tp_refs),
    ) {
        is_receiver_compatible_typed(index, &actual, &expected, &tp_smol)
    } else {
        false
    }
}

/// Recover concrete generic argument when a bytecode method signature is erased to Object or Any
pub fn recover_erased_generic_arg(
    index: &crate::indexer::WorkspaceIndex,
    rec_str: &str,
) -> Option<String> {
    let mut args = extract_type_arguments_list(rec_str);
    if args.is_empty() {
        let bare = rec_str.split('<').next().unwrap_or(rec_str).trim();
        for s in index.get_all_supertypes(bare) {
            let s_args = extract_type_arguments_list(&s);
            if !s_args.is_empty() {
                args = s_args;
                break;
            }
        }
    }
    args.last().and_then(|a| {
        let clean = a.trim();
        (!clean.is_empty() && StandardInterface::from_name(clean) != Some(StandardInterface::Any))
            .then(|| clean.to_string())
    })
}

/// Extract type parameter bindings for a generic container receiver
pub fn extract_container_type_param_bindings(
    index: &crate::indexer::WorkspaceIndex,
    receiver_type_str: &str,
) -> Vec<(smol_str::SmolStr, super::types::KotlinType)> {
    let mut bindings = Vec::new();
    let concrete_args = extract_type_arguments_list(receiver_type_str);
    if concrete_args.is_empty() {
        return bindings;
    }

    let bare = receiver_type_str
        .split('<')
        .next()
        .unwrap_or(receiver_type_str)
        .trim();
    let short_name = bare.rsplit('.').next().unwrap_or(bare);

    if let Some(decls) = index.name_to_decls.get(short_name) {
        for (_, decl) in decls {
            if matches!(
                decl.kind,
                SymbolKind::CLASS | SymbolKind::INTERFACE | SymbolKind::OBJECT
            ) && let Some(sig_info) = crate::inference::signature::SignatureInfo::from_decl(decl)
            {
                for (p, concrete) in sig_info.type_params.iter().zip(concrete_args.iter()) {
                    if let Some(k_ty) = super::types::parse_type_signature(concrete) {
                        bindings.push((p.name.clone(), k_ty));
                    }
                }
                if !sig_info.type_params.is_empty() {
                    return bindings;
                }
            }
        }
    }

    if concrete_args.len() == 1
        && let Some(k_ty) = super::types::parse_type_signature(&concrete_args[0])
    {
        bindings.push((smol_str::SmolStr::new_static("T"), k_ty));
    }
    bindings
}

pub fn extract_container_type_param_bindings_typed(
    index: &crate::indexer::WorkspaceIndex,
    receiver_type: &super::types::KotlinType,
) -> Vec<(smol_str::SmolStr, super::types::KotlinType)> {
    let mut bindings = Vec::new();
    let clean = receiver_type.unwrap_nullable_ref();
    let super::types::KotlinType::Class { name, args } = clean else {
        return bindings;
    };
    if args.is_empty() {
        return bindings;
    }
    let short_name = name.rsplit('.').next().unwrap_or(name);
    if let Some(decls) = index.name_to_decls.get(short_name) {
        for (_, decl) in decls {
            if matches!(
                decl.kind,
                tower_lsp::lsp_types::SymbolKind::CLASS
                    | tower_lsp::lsp_types::SymbolKind::INTERFACE
                    | tower_lsp::lsp_types::SymbolKind::OBJECT
            ) && let Some(sig_info) = crate::inference::signature::SignatureInfo::from_decl(decl)
            {
                for (param, concrete_arg) in sig_info.type_params.iter().zip(args.iter()) {
                    bindings.push((param.name.clone(), concrete_arg.type_ref.clone()));
                }
                if !sig_info.type_params.is_empty() {
                    return bindings;
                }
            }
        }
    }
    if args.len() == 1 {
        bindings.push((smol_str::SmolStr::new_static("T"), args[0].type_ref.clone()));
    }
    bindings
}

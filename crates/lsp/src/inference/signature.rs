use smol_str::SmolStr;

use crate::inference::types::{KotlinType, parse_type_signature_with_params};
use crate::syntax::symbol_item::DeclarationItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureParam {
    pub name: SmolStr,
    pub type_name: SmolStr,
    pub kotlin_type: Option<KotlinType>,
    pub has_default: bool,
    pub is_vararg: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenericParamInfo {
    pub name: SmolStr,
    pub bounds: Vec<KotlinType>,
    pub is_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureInfo {
    pub type_params: Vec<GenericParamInfo>,
    pub receiver: Option<KotlinType>,
    pub params: Vec<SignatureParam>,
    pub return_type: Option<KotlinType>,
}

impl SignatureInfo {
    pub fn parse(sig: &str) -> Option<Self> {
        let clean = sig.trim();
        if clean.is_empty() {
            return None;
        }

        if let Some(info) = super::signature_ast::parse_signature_from_ast(clean) {
            return Some(info);
        }

        let raw = crate::syntax::signature::parse_raw_signature(sig);
        let param_name_smols: Vec<SmolStr> =
            raw.type_params.iter().map(|p| p.name.clone()).collect();
        let param_names: Vec<&str> = param_name_smols.iter().map(|p| p.as_str()).collect();

        let type_params = raw
            .type_params
            .into_iter()
            .map(|p| {
                let bounds = p
                    .bounds
                    .into_iter()
                    .filter_map(|b| parse_type_signature_with_params(b.as_str(), &param_names))
                    .collect();
                GenericParamInfo {
                    name: p.name,
                    bounds,
                    is_out: p.is_out,
                }
            })
            .collect();

        let receiver = raw
            .receiver
            .and_then(|r| parse_type_signature_with_params(r.as_str(), &param_names));

        let params = raw
            .params
            .into_iter()
            .map(|p| {
                let kotlin_type =
                    parse_type_signature_with_params(p.type_name.as_str(), &param_names);
                SignatureParam {
                    name: p.name,
                    type_name: p.type_name,
                    kotlin_type,
                    has_default: p.has_default,
                    is_vararg: p.is_vararg,
                }
            })
            .collect();

        let return_type = raw
            .return_type
            .and_then(|ret| parse_type_signature_with_params(ret.as_str(), &param_names));

        Some(Self {
            type_params,
            receiver,
            params,
            return_type,
        })
    }

    pub fn from_decl(decl: &DeclarationItem) -> Option<Self> {
        decl.signature.as_deref().and_then(Self::parse)
    }
}

pub fn parse_signature_params(sig: &str) -> Vec<SignatureParam> {
    SignatureInfo::parse(sig)
        .map(|s| s.params)
        .unwrap_or_default()
}

pub fn get_signature_param_types(sig: Option<&str>) -> Vec<String> {
    sig.map(|s| {
        parse_signature_params(s)
            .into_iter()
            .map(|p| p.type_name.to_string())
            .collect()
    })
    .unwrap_or_default()
}

pub fn get_decl_param_types(decl: &DeclarationItem) -> Vec<String> {
    get_signature_param_types(decl.signature.as_deref())
}

fn is_type_name_any(name: &str) -> bool {
    super::types::standard::StandardInterface::from_name(name)
        == Some(super::types::standard::StandardInterface::Any)
}

fn find_top_level_arrow(type_str: &str) -> Option<usize> {
    let mut paren_depth = 0;
    let mut angle_depth = 0;
    let bytes = type_str.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        match bytes[idx] {
            b'(' => paren_depth += 1,
            b')' if paren_depth > 0 => paren_depth -= 1,
            b'<' => angle_depth += 1,
            b'>' if angle_depth > 0 => angle_depth -= 1,
            b'-' if paren_depth == 0
                && angle_depth == 0
                && idx + 1 < bytes.len()
                && bytes[idx + 1] == b'>' =>
            {
                return Some(idx);
            }
            _ => {}
        }
        idx += 1;
    }
    None
}

fn strip_enclosing_parens(mut text: &str) -> &str {
    text = text.trim();
    while text.starts_with('(') && text.ends_with(')') {
        let mut depth = 0;
        let mut matched = false;
        for (idx, ch) in text.char_indices() {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    if idx == text.len() - 1 {
                        matched = true;
                    }
                    break;
                }
            }
        }
        if matched {
            text = text[1..text.len() - 1].trim();
        } else {
            break;
        }
    }
    text
}

fn find_final_param_list_open(text: &str) -> Option<usize> {
    if !text.ends_with(')') {
        return None;
    }
    let mut paren_depth = 0;
    for (idx, ch) in text.char_indices().rev() {
        match ch {
            ')' => paren_depth += 1,
            '(' => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_functional_params(params_part: &str) -> (Option<&str>, &str) {
    let trimmed = params_part.trim();
    let without_suspend = trimmed
        .strip_prefix("suspend")
        .and_then(|rest| rest.strip_prefix(char::is_whitespace))
        .map(str::trim)
        .unwrap_or(trimmed);

    if let Some(open_paren_idx) = find_final_param_list_open(without_suspend) {
        let prefix = without_suspend[..open_paren_idx].trim();
        let paren_part = without_suspend[open_paren_idx..].trim();
        if let Some(receiver) = prefix.strip_suffix('.') {
            let clean_receiver = strip_enclosing_parens(receiver.trim());
            return (Some(clean_receiver), paren_part);
        }
        return (None, paren_part);
    }
    (None, without_suspend)
}

fn extract_param_type(param_str: &str) -> &str {
    let trimmed = param_str.trim();
    crate::syntax::signature::find_param_colon(trimmed)
        .map(|colon_idx| trimmed[colon_idx + 1..].trim())
        .unwrap_or(trimmed)
}

fn are_single_types_compatible(type_str1: &str, type_str2: &str) -> bool {
    let clean_type1 = strip_enclosing_parens(type_str1.trim_end_matches('?').trim());
    let clean_type2 = strip_enclosing_parens(type_str2.trim_end_matches('?').trim());
    if clean_type1 == clean_type2 {
        return true;
    }
    if let (Some(arrow_idx1), Some(arrow_idx2)) = (
        find_top_level_arrow(clean_type1),
        find_top_level_arrow(clean_type2),
    ) {
        let (receiver1, raw_params1) = parse_functional_params(&clean_type1[..arrow_idx1]);
        let (receiver2, raw_params2) = parse_functional_params(&clean_type2[..arrow_idx2]);
        let return_type1 = clean_type1[arrow_idx1 + 2..].trim();
        let return_type2 = clean_type2[arrow_idx2 + 2..].trim();

        let mut params1 = if raw_params1.is_empty() {
            Vec::new()
        } else {
            let stripped = strip_enclosing_parens(raw_params1);
            if stripped.is_empty() {
                Vec::new()
            } else {
                crate::syntax::signature::split_top_level_csv(stripped)
            }
        };
        let mut params2 = if raw_params2.is_empty() {
            Vec::new()
        } else {
            let stripped = strip_enclosing_parens(raw_params2);
            if stripped.is_empty() {
                Vec::new()
            } else {
                crate::syntax::signature::split_top_level_csv(stripped)
            }
        };

        if let Some(rec1) = receiver1 {
            params1.insert(0, rec1.to_string());
        }
        if let Some(rec2) = receiver2 {
            params2.insert(0, rec2.to_string());
        }

        if params1.len() != params2.len() {
            return false;
        }
        let params_compatible = params1.iter().zip(params2.iter()).all(|(param1, param2)| {
            are_single_types_compatible(extract_param_type(param1), extract_param_type(param2))
        });

        return params_compatible && are_single_types_compatible(return_type1, return_type2);
    }

    let simple_name1 = clean_type1.rsplit('.').next().unwrap_or(clean_type1);
    let simple_name2 = clean_type2.rsplit('.').next().unwrap_or(clean_type2);
    simple_name1 == simple_name2 || is_type_name_any(simple_name1) || is_type_name_any(simple_name2)
}

pub fn are_param_types_compatible(types1: &[String], types2: &[String]) -> bool {
    if types1.len() != types2.len() {
        return false;
    }
    types1
        .iter()
        .zip(types2.iter())
        .all(|(type_str1, type_str2)| are_single_types_compatible(type_str1, type_str2))
}

pub fn are_unpassed_parameters_compatible(
    params: &[SignatureParam],
    target_idx: usize,
    preceding_arg_count: usize,
) -> bool {
    if target_idx >= params.len() {
        return false;
    }
    let has_vararg_before = params[..target_idx].iter().any(|param| param.is_vararg);
    if preceding_arg_count > target_idx && !has_vararg_before {
        return false;
    }
    if preceding_arg_count < target_idx {
        for param in &params[preceding_arg_count..target_idx] {
            if !param.has_default && !param.is_vararg {
                return false;
            }
        }
    }
    for param in &params[target_idx + 1..] {
        if !param.has_default && !param.is_vararg {
            return false;
        }
    }
    true
}

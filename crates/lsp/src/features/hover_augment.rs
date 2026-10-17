use tower_lsp::lsp_types::Url;

use super::hover::HoverContext;
use crate::config::VariableHoverTypeInference;
use crate::indexer::WorkspaceIndex;
use crate::inference::types::KotlinType;
use crate::inference::{
    TraceCollector, Tracer, infer_variable_type, infer_variable_type_traced, resolve_full_type_name,
};

pub fn augment_variable_signature_with_type(
    sig: &str,
    var_name: &str,
    inferred: Option<&KotlinType>,
    opt: VariableHoverTypeInference,
    index: &WorkspaceIndex,
    current_url: &Url,
) -> String {
    if matches!(opt, VariableHoverTypeInference::Auto) {
        return sig.to_string();
    }

    let Some((name_start, name_end)) = find_declared_name_position(sig, var_name) else {
        return sig.to_string();
    };

    let before_name = &sig[..name_start];
    let after_name = &sig[name_end..];
    let trimmed_after = after_name.trim_start();

    if let Some(after_colon) = trimmed_after.strip_prefix(':') {
        if matches!(
            opt,
            VariableHoverTypeInference::Always | VariableHoverTypeInference::AlwaysWithTrace
        ) {
            sig.to_string()
        } else {
            augment_explicit_type_signature(
                before_name,
                var_name,
                after_colon,
                inferred,
                opt,
                index,
                current_url,
            )
        }
    } else {
        let type_str = determine_inferred_type_string(inferred, opt, index, current_url);
        format!("{}{}: {}{}", before_name, var_name, type_str, after_name)
    }
}

fn augment_explicit_type_signature(
    before_name: &str,
    var_name: &str,
    after_colon: &str,
    inferred: Option<&KotlinType>,
    opt: VariableHoverTypeInference,
    index: &WorkspaceIndex,
    current_url: &Url,
) -> String {
    match opt {
        VariableHoverTypeInference::AlwaysFullName => {
            let after_colon_trimmed = after_colon.trim_start();
            let type_len = after_colon_trimmed
                .find(['=', '{', '\n', '\r'])
                .or_else(|| after_colon_trimmed.find(" by "))
                .unwrap_or(after_colon_trimmed.len());
            let explicit_type = after_colon_trimmed[..type_len].trim();
            let rest = after_colon_trimmed[type_len..].trim();

            let full_type = if let Some(t) = inferred {
                let full = resolve_full_type_name(index, current_url, &t.to_string());
                if t.is_nullable() && !full.ends_with('?') {
                    format!("{}?", full)
                } else {
                    full
                }
            } else {
                let is_nullable = explicit_type.ends_with('?');
                let base = explicit_type.trim_end_matches('?').trim();
                let full = resolve_full_type_name(index, current_url, base);
                if is_nullable && !full.ends_with('?') {
                    format!("{}?", full)
                } else {
                    full
                }
            };

            if rest.is_empty() {
                format!("{}{}: {}", before_name, var_name, full_type)
            } else {
                format!("{}{}: {} {}", before_name, var_name, full_type, rest)
            }
        }
        _ => format!("{}{}:{}", before_name, var_name, after_colon),
    }
}

fn determine_inferred_type_string(
    inferred: Option<&KotlinType>,
    opt: VariableHoverTypeInference,
    index: &WorkspaceIndex,
    current_url: &Url,
) -> String {
    match inferred {
        Some(t) => {
            let base = match opt {
                VariableHoverTypeInference::Always
                | VariableHoverTypeInference::AlwaysWithTrace => t.to_short_string(),
                VariableHoverTypeInference::AlwaysFullName => {
                    resolve_full_type_name(index, current_url, &t.to_string())
                }
                VariableHoverTypeInference::Auto => unreachable!(),
            };
            if t.is_nullable() && !base.ends_with('?') {
                format!("{}?", base)
            } else {
                base
            }
        }
        None => "?".to_string(),
    }
}

pub fn find_declared_name_position(sig: &str, var_name: &str) -> Option<(usize, usize)> {
    let mut search_start = 0;
    while let Some(idx) = sig[search_start..].find(var_name) {
        let abs_start = search_start + idx;
        let abs_end = abs_start + var_name.len();

        let prev_ok = if abs_start == 0 {
            true
        } else {
            let prev_char = sig[..abs_start].chars().next_back().unwrap();
            prev_char.is_whitespace() || prev_char == '(' || prev_char == ',' || prev_char == '`'
        };

        let next_ok = if abs_end >= sig.len() {
            true
        } else {
            let next_char = sig[abs_end..].chars().next().unwrap();
            next_char.is_whitespace()
                || next_char == ':'
                || next_char == '='
                || next_char == ')'
                || next_char == ','
                || next_char == '`'
        };

        if prev_ok && next_ok {
            let before = &sig[..abs_start];
            let quote_count = before.chars().filter(|&c| c == '"').count();
            if quote_count % 2 == 0 {
                return Some((abs_start, abs_end));
            }
        }

        search_start = abs_start + 1;
    }
    None
}

pub fn infer_variable_and_augment_signature(
    ctx: HoverContext<'_>,
    var_name: &str,
    sig: &mut Option<String>,
) -> Option<String> {
    let (inferred, trace) = if ctx.type_infer_opt.is_trace_enabled() {
        let mut collector = TraceCollector::new();
        let mut tracer = Tracer::Active(&mut collector);
        let res = infer_variable_type_traced(
            ctx.index,
            ctx.current_url,
            var_name,
            ctx.pos,
            ctx.source,
            ctx.root,
            &mut tracer,
        );
        (res, Some(collector.render_markdown()))
    } else {
        let res = infer_variable_type(
            ctx.index,
            ctx.current_url,
            var_name,
            ctx.pos,
            ctx.source,
            ctx.root,
        );
        (res, None)
    };

    if let Some(s) = sig.as_ref() {
        *sig = Some(augment_variable_signature_with_type(
            s,
            var_name,
            inferred.as_ref(),
            ctx.type_infer_opt,
            ctx.index,
            ctx.current_url,
        ));
    }
    trace
}

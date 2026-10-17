use tower_lsp::lsp_types::{Position, Url};
use tree_sitter::Node;

use super::hover::HoverContext;
use super::hover_format::{
    build_qualifying_path, find_enclosing_container_name, format_markdown_hover,
    is_extension_signature,
};
use crate::config::VariableHoverTypeInference;
use crate::doc::{extract_doc_comment, extract_param_doc_from_function, merge_extra_comments};
use crate::indexer::WorkspaceIndex;
use crate::parser::{
    extract_identifier_text, extract_signature_and_comments, find_enclosing_declaration_node,
};

pub fn is_local_scope(node: &Node) -> bool {
    let mut curr = node.parent();
    while let Some(parent) = curr {
        if matches!(
            parent.kind(),
            "function_declaration"
                | "function_body"
                | "lambda_literal"
                | "anonymous_initializer"
                | "secondary_constructor"
                | "getter"
                | "setter"
                | "for_statement"
                | "while_statement"
                | "do_while_statement"
                | "if_expression"
                | "when_expression"
                | "catch_block"
        ) {
            return true;
        }
        if matches!(
            parent.kind(),
            "class_declaration" | "object_declaration" | "interface_declaration" | "source_file"
        ) {
            return false;
        }
        curr = parent.parent();
    }
    false
}

pub fn find_enclosing_constructor_or_func<'a>(node: &Node<'a>) -> Option<Node<'a>> {
    let mut curr = node.parent();
    while let Some(parent) = curr {
        if matches!(
            parent.kind(),
            "function_declaration" | "class_declaration" | "secondary_constructor"
        ) {
            return Some(parent);
        }
        curr = parent.parent();
    }
    None
}

pub fn extract_catch_param_signature(node: Node, source: &str) -> Option<String> {
    let catch = if node.kind() == "catch_block" {
        Some(node)
    } else {
        node.parent().filter(|p| p.kind() == "catch_block")
    }?;
    let mut text = String::new();
    let mut inside = false;
    let mut cursor = catch.walk();
    for child in catch.children(&mut cursor) {
        let k = child.kind();
        if k == "(" {
            inside = true;
        } else if k == ")" {
            break;
        } else if inside && let Ok(t) = child.utf8_text(source.as_bytes()) {
            if !text.is_empty() && !text.ends_with(':') && t != ":" {
                text.push(' ');
            }
            text.push_str(t);
        }
    }
    (!text.is_empty()).then(|| format!("val {}", text))
}

pub fn format_local_node_markdown(
    ctx: HoverContext<'_>,
    node_at_loc: Node,
    package_name: Option<&str>,
    symbol_name: &str,
) -> String {
    if let Some(catch_sig) = extract_catch_param_signature(node_at_loc, ctx.source) {
        return format!("```kotlin\n{}\n```\n", catch_sig);
    }
    let local_node = find_enclosing_declaration_node(node_at_loc);
    let sig_and_comments = extract_signature_and_comments(&local_node, ctx.source);
    let mut sig = sig_and_comments.as_ref().map(|(s, _)| s.clone());
    let mut doc = extract_doc_comment(&local_node, ctx.source);

    let is_param = matches!(local_node.kind(), "parameter" | "class_parameter");
    let is_var_decl = local_node.kind() == "variable_declaration"
        || (local_node.kind() == "property_declaration" && is_local_scope(&local_node));
    if is_param || is_var_decl {
        adjust_local_sig_and_doc(&local_node, ctx.source, &mut sig, &mut doc);
    }

    let trace_rendered = if (is_param || is_var_decl || local_node.kind() == "property_declaration")
        && ctx.type_infer_opt.is_enabled()
    {
        let var_name = extract_identifier_text(&local_node, ctx.source).unwrap_or(symbol_name);
        super::hover_augment::infer_variable_and_augment_signature(ctx, var_name, &mut sig)
    } else {
        None
    };

    if let Some((_, ref extra_comments)) = sig_and_comments {
        doc = merge_extra_comments(doc, extra_comments);
    }

    let path = if is_param || is_var_decl {
        None
    } else {
        let is_ext = sig
            .as_ref()
            .and_then(|s| extract_identifier_text(&local_node, ctx.source).map(|n| (s, n)))
            .is_some_and(|(s, n)| is_extension_signature(s, n));

        let container = if is_ext {
            None
        } else {
            find_enclosing_container_name(&local_node, ctx.source)
        };
        build_qualifying_path(package_name, container.as_deref())
    };

    let mut markdown = format_markdown_hover(path.as_deref(), sig.as_deref(), doc.as_deref());
    if let Some(t) = trace_rendered {
        markdown.push_str(&t);
    }
    markdown
}

fn adjust_local_sig_and_doc(
    local_node: &Node,
    source: &str,
    sig: &mut Option<String>,
    doc: &mut Option<String>,
) {
    if let Some(s) = sig.as_deref()
        && !s.starts_with("val ")
        && !s.starts_with("var ")
        && !s.starts_with("fun ")
    {
        let prefix = if is_mutable_local_variable(local_node) {
            "var "
        } else {
            "val "
        };
        *sig = Some(crate::concat_str!(prefix, s));
    }
    if doc.is_none()
        && let Some(parent) = find_enclosing_constructor_or_func(local_node)
        && let Some(name) = extract_identifier_text(local_node, source)
    {
        *doc = extract_param_doc_from_function(&parent, name, source);
    }
}

fn is_mutable_local_variable(node: &Node) -> bool {
    let mut curr = Some(*node);
    while let Some(n) = curr {
        let k = n.kind();
        if k == "property_declaration" || k == "destructuring_declaration" {
            let mut cursor = n.walk();
            for child in n.children(&mut cursor) {
                if child.kind() == "var" || child.child(0).is_some_and(|c| c.kind() == "var") {
                    return true;
                }
                if child.kind() == "val" || child.child(0).is_some_and(|c| c.kind() == "val") {
                    return false;
                }
            }
            break;
        }
        if matches!(
            k,
            "function_declaration" | "class_declaration" | "source_file"
        ) {
            break;
        }
        curr = n.parent();
    }
    false
}

fn is_member_selector(node: Node) -> bool {
    if let Some(parent) = node.parent() {
        let pk = parent.kind();
        if pk == "navigation_suffix" || pk == "callable_reference" {
            return true;
        }
        if pk == "navigation_expression" && parent.child(0).is_some_and(|c| c.id() != node.id()) {
            return true;
        }
    }
    false
}

pub fn format_implicit_it_hover(
    index: &WorkspaceIndex,
    current_url: &Url,
    pos: Position,
    source: &str,
    root: Option<Node>,
    type_infer_opt: VariableHoverTypeInference,
) -> Option<String> {
    let root_node = root?;
    let target_node = crate::parser::find_node_at_position(root_node, pos)?;
    let text = target_node.utf8_text(source.as_bytes()).ok()?.trim();
    if text != "it" || is_member_selector(target_node) {
        return None;
    }

    let lambdas = crate::inference::collect_enclosing_lambdas(target_node);
    if lambdas.is_empty() {
        return None;
    }

    let (inferred_type, trace_md) = if type_infer_opt.is_trace_enabled() {
        let mut collector = crate::inference::TraceCollector::new();
        let mut tracer = crate::inference::Tracer::Active(&mut collector);
        let ty = crate::inference::resolve_it_keyword_traced(
            index,
            current_url,
            target_node,
            pos,
            source,
            Some(root_node),
            0,
            &mut tracer,
        );
        (ty, Some(collector.render_markdown()))
    } else {
        let ty = crate::inference::resolve_it_keyword(
            index,
            current_url,
            target_node,
            pos,
            source,
            Some(root_node),
            0,
        );
        (ty, None)
    };

    let inferred_type = inferred_type?;
    let type_str =
        format_inferred_it_type_string(&inferred_type, index, current_url, type_infer_opt);
    let sig = crate::concat_str!("val it: ", type_str);
    let mut markdown = format_markdown_hover(None, Some(&sig), None);
    if let Some(trace) = trace_md {
        markdown.push_str(&trace);
    }
    Some(markdown)
}

fn format_inferred_it_type_string(
    inferred_type: &crate::inference::KotlinType,
    index: &WorkspaceIndex,
    current_url: &Url,
    type_infer_opt: VariableHoverTypeInference,
) -> String {
    match type_infer_opt {
        VariableHoverTypeInference::AlwaysFullName => {
            let full = crate::inference::resolve_full_type_name(
                index,
                current_url,
                &inferred_type.to_string(),
            );
            if inferred_type.is_nullable() && !full.ends_with('?') {
                crate::concat_str!(full, "?")
            } else {
                full
            }
        }
        _ => {
            let short = inferred_type.to_short_string();
            if inferred_type.is_nullable() && !short.ends_with('?') {
                crate::concat_str!(short, "?")
            } else {
                short
            }
        }
    }
}

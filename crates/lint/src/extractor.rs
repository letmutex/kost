//! AST property extraction for lexical scopes.

use crate::annotation::extract_annotations;
use crate::context::{LintCtx, ScopeFrame};
use crate::types::{PropertyKey, PropertyValue, ReportTarget, Scope, TextRange};
use crate::util::{count_function_parameters, find_child_identifier, find_child_identifier_node};
use smol_str::SmolStr;

pub fn populate_file_properties(node: &tree_sitter::Node, ctx: &mut LintCtx) {
    if ctx
        .registry
        .needed_properties
        .contains(&PropertyKey::FileLines)
    {
        let start_pos = node.start_position();
        let end_pos = node.end_position();
        let lines = if node.start_byte() == node.end_byte() {
            0
        } else if end_pos.column == 0 && end_pos.row > start_pos.row {
            (end_pos.row - start_pos.row) as i64
        } else {
            (end_pos.row - start_pos.row + 1) as i64
        };
        ctx.scope_tracker
            .insert_property(PropertyKey::FileLines, PropertyValue::Int(lines));
    }
}

pub fn populate_function_properties(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let needed = &ctx.registry.needed_properties;

    if needed.contains(&PropertyKey::FunctionLines) {
        let lines = (node.end_position().row - node.start_position().row + 1) as i64;
        ctx.scope_tracker
            .insert_property(PropertyKey::FunctionLines, PropertyValue::Int(lines));
    }

    populate_name_and_length(
        node,
        source,
        ctx,
        PropertyKey::FunctionName,
        PropertyKey::FunctionNameLength,
    );

    if needed.contains(&PropertyKey::FunctionParametersCount) {
        let count = count_function_parameters(node);
        ctx.scope_tracker.insert_property(
            PropertyKey::FunctionParametersCount,
            PropertyValue::Int(count),
        );
    }

    if needed.contains(&PropertyKey::FunctionAnnotations) {
        let annotations = extract_annotations(node, source);
        ctx.scope_tracker.insert_property(
            PropertyKey::FunctionAnnotations,
            PropertyValue::StrList(annotations),
        );
    }
}

pub fn populate_class_properties(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let needed = &ctx.registry.needed_properties;

    if needed.contains(&PropertyKey::ClassLines) {
        let lines = (node.end_position().row - node.start_position().row + 1) as i64;
        ctx.scope_tracker
            .insert_property(PropertyKey::ClassLines, PropertyValue::Int(lines));
    }

    populate_name_and_length(
        node,
        source,
        ctx,
        PropertyKey::ClassName,
        PropertyKey::ClassNameLength,
    );

    if needed.contains(&PropertyKey::ClassMethodsCount) {
        let count = count_class_methods(node);
        ctx.scope_tracker
            .insert_property(PropertyKey::ClassMethodsCount, PropertyValue::Int(count));
    }

    if needed.contains(&PropertyKey::ClassAnnotations) {
        let annotations = extract_annotations(node, source);
        ctx.scope_tracker.insert_property(
            PropertyKey::ClassAnnotations,
            PropertyValue::StrList(annotations),
        );
    }
}

pub fn populate_variable_properties(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let needed = &ctx.registry.needed_properties;

    if needed.contains(&PropertyKey::VariableLines) {
        let lines = (node.end_position().row - node.start_position().row + 1) as i64;
        ctx.scope_tracker
            .insert_property(PropertyKey::VariableLines, PropertyValue::Int(lines));
    }

    populate_name_and_length(
        node,
        source,
        ctx,
        PropertyKey::VariableName,
        PropertyKey::VariableNameLength,
    );
}

fn populate_name_and_length(
    node: tree_sitter::Node,
    source: &[u8],
    ctx: &mut LintCtx,
    name_key: PropertyKey,
    len_key: PropertyKey,
) {
    let needed = &ctx.registry.needed_properties;
    let needs_name = needed.contains(&name_key);
    let needs_len = needed.contains(&len_key);
    if (needs_name || needs_len)
        && let Some(name) = find_child_identifier(node, source)
    {
        if needs_len {
            let len = identifier_clean_length(name.as_str());
            ctx.scope_tracker
                .insert_property(len_key, PropertyValue::Int(len));
        }
        if needs_name {
            ctx.scope_tracker
                .insert_property(name_key, PropertyValue::Str(name));
        }
    }
}

pub fn populate_lambda_properties(node: tree_sitter::Node, ctx: &mut LintCtx) {
    let needed = &ctx.registry.needed_properties;

    if needed.contains(&PropertyKey::LambdaLines) {
        let lines = (node.end_position().row - node.start_position().row + 1) as i64;
        ctx.scope_tracker
            .insert_property(PropertyKey::LambdaLines, PropertyValue::Int(lines));
    }
}

pub fn populate_import_properties(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let needed = &ctx.registry.needed_properties;

    let is_wildcard = node
        .children(&mut node.walk())
        .any(|c| c.kind() == "wildcard_import");
    if needed.contains(&PropertyKey::ImportIsWildcard) {
        ctx.scope_tracker.insert_property(
            PropertyKey::ImportIsWildcard,
            PropertyValue::Bool(is_wildcard),
        );
    }

    let mut id_text = "";
    let mut alias_text = "";
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" {
            if let Ok(t) = child.utf8_text(source) {
                id_text = t;
            }
        } else if child.kind() == "import_alias" {
            let mut alias_cursor = child.walk();
            for alias_child in child.children(&mut alias_cursor) {
                if matches!(alias_child.kind(), "simple_identifier" | "type_identifier")
                    && let Ok(t) = alias_child.utf8_text(source)
                {
                    alias_text = t;
                }
            }
        }
    }

    if needed.contains(&PropertyKey::ImportPath) {
        let path = if is_wildcard {
            common::concat_smol_str!(id_text, ".*")
        } else {
            SmolStr::new(id_text)
        };
        ctx.scope_tracker
            .insert_property(PropertyKey::ImportPath, PropertyValue::Str(path));
    }

    if needed.contains(&PropertyKey::ImportAlias) {
        ctx.scope_tracker.insert_property(
            PropertyKey::ImportAlias,
            PropertyValue::Str(SmolStr::new(alias_text)),
        );
    }
}

pub fn enter_scope_if_matching(
    node: tree_sitter::Node,
    source: &[u8],
    ctx: &mut LintCtx,
) -> Option<Scope> {
    let kind = node.kind();
    let range = TextRange::from_node(node);

    match kind {
        "function_declaration" => {
            ctx.scope_tracker.push(Scope::Function, range);
            if let Some(frame) = ctx.scope_tracker.current_mut() {
                attach_declaration_anchors(
                    frame,
                    range,
                    ReportTarget::FunctionStart,
                    ReportTarget::FunctionEnd,
                    ReportTarget::FunctionName,
                    node,
                );
            }
            populate_function_properties(node, source, ctx);
            Some(Scope::Function)
        }
        "class_declaration" | "object_declaration" => {
            ctx.scope_tracker.push(Scope::Class, range);
            if let Some(frame) = ctx.scope_tracker.current_mut() {
                attach_declaration_anchors(
                    frame,
                    range,
                    ReportTarget::ClassStart,
                    ReportTarget::ClassEnd,
                    ReportTarget::ClassName,
                    node,
                );
            }
            populate_class_properties(node, source, ctx);
            Some(Scope::Class)
        }
        "property_declaration" => {
            ctx.scope_tracker.push(Scope::Variable, range);
            if let Some(frame) = ctx.scope_tracker.current_mut() {
                attach_declaration_anchors(
                    frame,
                    range,
                    ReportTarget::VariableStart,
                    ReportTarget::VariableEnd,
                    ReportTarget::VariableName,
                    node,
                );
            }
            populate_variable_properties(node, source, ctx);
            Some(Scope::Variable)
        }
        "lambda_literal" => {
            ctx.scope_tracker.push(Scope::Lambda, range);
            populate_lambda_properties(node, ctx);
            Some(Scope::Lambda)
        }
        "import_header" => {
            ctx.scope_tracker.push(Scope::Import, range);
            populate_import_properties(node, source, ctx);
            Some(Scope::Import)
        }
        _ => None,
    }
}

fn attach_declaration_anchors(
    frame: &mut ScopeFrame,
    range: TextRange,
    start_target: ReportTarget,
    end_target: ReportTarget,
    name_target: ReportTarget,
    node: tree_sitter::Node,
) {
    frame.insert_target_anchor(
        start_target,
        TextRange::new(
            range.start_line,
            range.start_col,
            range.start_line,
            range.start_col,
        ),
    );
    frame.insert_target_anchor(
        end_target,
        TextRange::new(range.end_line, range.end_col, range.end_line, range.end_col),
    );
    if let Some(id_node) = find_child_identifier_node(node) {
        frame.insert_target_anchor(name_target, TextRange::from_node(id_node));
    }
}

fn identifier_clean_length(raw_name: &str) -> i64 {
    let clean = raw_name
        .strip_prefix('`')
        .and_then(|s| s.strip_suffix('`'))
        .unwrap_or(raw_name);
    clean.chars().count() as i64
}

fn count_class_methods(node: tree_sitter::Node) -> i64 {
    let mut count = 0;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "class_body" {
            let mut body_cursor = child.walk();
            for body_child in child.children(&mut body_cursor) {
                if body_child.kind() == "function_declaration" {
                    count += 1;
                }
            }
        }
    }
    count
}

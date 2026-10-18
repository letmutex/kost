//! AST property extraction for lexical scopes.

use crate::context::LintCtx;
use crate::types::{PropertyKey, PropertyValue, Scope, TextRange};
use crate::util::{count_function_parameters, find_child_identifier};
use smallvec::SmallVec;
use smol_str::SmolStr;

pub fn populate_file_properties(node: &tree_sitter::Node, ctx: &mut LintCtx) {
    if ctx
        .registry
        .needed_properties
        .contains(&PropertyKey::FileLines)
    {
        let lines = (node.end_position().row - node.start_position().row + 1) as i64;
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

    if needed.contains(&PropertyKey::FunctionName)
        && let Some(name) = find_child_identifier(node, source)
    {
        ctx.scope_tracker
            .insert_property(PropertyKey::FunctionName, PropertyValue::Str(name));
    }

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

    if needed.contains(&PropertyKey::ClassName)
        && let Some(name) = find_child_identifier(node, source)
    {
        ctx.scope_tracker
            .insert_property(PropertyKey::ClassName, PropertyValue::Str(name));
    }

    if needed.contains(&PropertyKey::ClassAnnotations) {
        let annotations = extract_annotations(node, source);
        ctx.scope_tracker.insert_property(
            PropertyKey::ClassAnnotations,
            PropertyValue::StrList(annotations),
        );
    }
}

pub fn populate_property_properties(node: tree_sitter::Node, source: &[u8], ctx: &mut LintCtx) {
    let needed = &ctx.registry.needed_properties;

    if needed.contains(&PropertyKey::PropertyLines) {
        let lines = (node.end_position().row - node.start_position().row + 1) as i64;
        ctx.scope_tracker
            .insert_property(PropertyKey::PropertyLines, PropertyValue::Int(lines));
    }

    if needed.contains(&PropertyKey::PropertyName)
        && let Some(name) = find_child_identifier(node, source)
    {
        ctx.scope_tracker
            .insert_property(PropertyKey::PropertyName, PropertyValue::Str(name));
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
            crate::concat_smol_str!(id_text, ".*")
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

pub fn extract_annotations(node: tree_sitter::Node, source: &[u8]) -> SmallVec<[SmolStr; 4]> {
    let mut annotations = SmallVec::new();
    let Some(modifiers) = node
        .children(&mut node.walk())
        .find(|child| child.kind() == "modifiers")
    else {
        return annotations;
    };

    let mut mod_cursor = modifiers.walk();
    for modifier in modifiers.children(&mut mod_cursor) {
        if modifier.kind() != "annotation" {
            continue;
        }

        let mut ann_cursor = modifier.walk();
        for ann_child in modifier.children(&mut ann_cursor) {
            if let Some(user_type) = find_annotation_type(ann_child)
                && let Ok(full_name) = user_type.utf8_text(source)
            {
                let trimmed = full_name.trim();
                push_annotation(&mut annotations, trimmed);
                let short_name = trimmed.rsplit('.').next().unwrap_or(trimmed);
                if short_name != trimmed {
                    push_annotation(&mut annotations, short_name);
                }
            }
        }
    }

    annotations
}

fn find_annotation_type(node: tree_sitter::Node) -> Option<tree_sitter::Node> {
    if node.kind() == "user_type" {
        Some(node)
    } else if node.kind() == "constructor_invocation" {
        node.named_child(0)
            .filter(|child| child.kind() == "user_type")
    } else {
        None
    }
}

fn push_annotation(annotations: &mut SmallVec<[SmolStr; 4]>, name: &str) {
    let name_smol = SmolStr::new(name);
    if !annotations.contains(&name_smol) {
        let at_name = crate::concat_smol_str!("@", name);
        annotations.push(name_smol);
        annotations.push(at_name);
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
            populate_function_properties(node, source, ctx);
            Some(Scope::Function)
        }
        "class_declaration" | "object_declaration" => {
            ctx.scope_tracker.push(Scope::Class, range);
            populate_class_properties(node, source, ctx);
            Some(Scope::Class)
        }
        "property_declaration" => {
            ctx.scope_tracker.push(Scope::Property, range);
            populate_property_properties(node, source, ctx);
            Some(Scope::Property)
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

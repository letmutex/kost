pub mod accessors;
pub mod annotation;
pub mod assignment;
pub mod binary;
pub mod callable_reference;
pub mod calls;
pub mod chain;
pub mod class_body;
pub mod comma_list;
pub mod comment;
pub mod constructors;
pub mod context;
pub mod context_receiver;
pub mod control;
pub mod decl;
pub mod delegates;
pub mod destructuring;
pub mod elvis;
pub mod expr;
pub mod expression_body;
pub mod func;
pub mod imports;
pub mod lambda;
pub mod lists;
pub mod loops;
pub mod navigation;
pub mod params;
pub mod property;
pub mod property_header;
pub mod scoping;
pub mod secondary_constructor;
pub mod source_file;
pub mod strings;
pub mod try_catch;
pub mod type_ops;
pub mod types;
pub mod when_expr;

use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::accessors::lower_getter_setter;
use crate::lower::annotation::{
    lower_annotated_lambda, lower_annotation, lower_file_annotation, lower_modifiers,
};
use crate::lower::assignment::{lower_assignment, lower_directly_assignable_expression};
use crate::lower::binary::lower_binary_expression;
use crate::lower::callable_reference::lower_callable_reference;
use crate::lower::calls::{lower_call_expression, lower_call_suffix};
use crate::lower::class_body::{lower_anonymous_initializer, lower_companion_object};
use crate::lower::comment::lower_comment;
use crate::lower::constructors::lower_primary_constructor;
use crate::lower::context::LowerContext;
use crate::lower::context_receiver::lower_context_receiver;
use crate::lower::control::{
    lower_control_structure_body, lower_if_expression, lower_jump_expression,
    lower_type_or_range_test,
};
use crate::lower::decl::{lower_class_body, lower_class_declaration};
use crate::lower::destructuring::{lower_backing_field, lower_destructuring_declaration};
use crate::lower::elvis::{find_leftmost_unparenthesized_elvis, lower_unparenthesized_elvis_chain};
use crate::lower::expr::{lower_block, lower_statements};
use crate::lower::func::{lower_function_body, lower_function_declaration};
use crate::lower::imports::{lower_import_list, lower_package_header};
use crate::lower::lambda::{lower_lambda_literal, lower_lambda_parameters};
use crate::lower::lists::{lower_collection_literal, lower_list_with_commas};
use crate::lower::loops::{lower_do_while_statement, lower_for_statement, lower_while_statement};
use crate::lower::navigation::{
    is_spine_root, lower_navigation_expression, lower_navigation_suffix,
};
use crate::lower::params::{
    lower_multi_variable_declaration, lower_parameter, lower_value_argument,
};
use crate::lower::property::lower_property_declaration;
use crate::lower::secondary_constructor::lower_secondary_constructor;
pub use crate::lower::source_file::lower_source_file;
use crate::lower::strings::lower_string_literal;
use crate::lower::try_catch::{lower_catch_block, lower_finally_block, lower_try_expression};
use crate::lower::type_ops::{
    lower_as_or_check_expression, lower_indexing_expression, lower_postfix_expression,
    lower_prefix_expression,
};
use crate::lower::types::{
    lower_delegation_specifier, lower_enum_entry, lower_function_type, lower_not_nullable_type,
    lower_type_alias, lower_type_constraints, lower_type_list, lower_type_parameter,
};
use crate::lower::when_expr::{lower_when_entry, lower_when_expression, lower_when_subject};

fn lower_declaration_node<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Option<Doc<'a>> {
    let doc = match node.kind() {
        "file_annotation" => lower_file_annotation(node, ctx),
        "package_header" => lower_package_header(node, ctx),
        "import_list" => lower_import_list(node, ctx),
        "context_receiver" => lower_context_receiver(node, ctx),
        "class_declaration" | "object_declaration" | "object_literal" => {
            lower_class_declaration(node, ctx)
        }
        "modifiers"
        | "parameter_modifiers"
        | "type_modifiers"
        | "type_projection_modifiers"
        | "type_parameter_modifiers" => lower_modifiers(node, ctx),
        "companion_object" => lower_companion_object(node, ctx),
        "anonymous_initializer" => lower_anonymous_initializer(node, ctx),
        "class_body" | "enum_class_body" => lower_class_body(node, ctx),
        "primary_constructor" => lower_primary_constructor(node, ctx),
        "secondary_constructor" => lower_secondary_constructor(node, ctx),
        "function_declaration" | "anonymous_function" => lower_function_declaration(node, ctx),
        "function_body" => lower_function_body(node, ctx),
        "getter" | "setter" => lower_getter_setter(node, ctx),
        "backing_field" => lower_backing_field(node, ctx),
        "destructuring_declaration" => lower_destructuring_declaration(node, ctx),
        "property_declaration" => lower_property_declaration(node, ctx),
        "multi_variable_declaration" => lower_multi_variable_declaration(node, ctx),
        "parameter"
        | "class_parameter"
        | "variable_declaration"
        | "parameter_with_optional_type" => lower_parameter(node, ctx),
        "enum_entry" => lower_enum_entry(node, ctx),
        "type_alias" => lower_type_alias(node, ctx),
        _ => return None,
    };
    Some(doc)
}

fn lower_expression_node<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Option<Doc<'a>> {
    let doc = match node.kind() {
        "block" => lower_block(node, ctx),
        "statements" => lower_statements(node, ctx),
        "call_expression" => lower_call_expression(node, ctx),
        "indexing_expression" => lower_indexing_expression(node, ctx),
        "call_suffix" => lower_call_suffix(node, ctx),
        "callable_reference" => lower_callable_reference(node, ctx),
        "navigation_expression" => lower_navigation_expression(node, ctx),
        "navigation_suffix" => lower_navigation_suffix(node, ctx),
        "as_expression" | "check_expression" => lower_as_or_check_expression(node, ctx),
        "binary_expression"
        | "additive_expression"
        | "multiplicative_expression"
        | "comparison_expression"
        | "equality_expression"
        | "conjunction_expression"
        | "disjunction_expression"
        | "infix_expression"
        | "elvis_expression" => lower_binary_expression(node, ctx),
        "jump_expression" => lower_jump_expression(node, ctx),
        "when_expression" => lower_when_expression(node, ctx),
        "when_entry" => lower_when_entry(node, ctx),
        "when_subject" => lower_when_subject(node, ctx),
        "annotated_lambda" => lower_annotated_lambda(node, ctx),
        "lambda_literal" => lower_lambda_literal(node, ctx),
        "lambda_parameters" => lower_lambda_parameters(node, ctx).0,
        "function_value_parameters"
        | "class_parameters"
        | "value_arguments"
        | "indexing_suffix"
        | "function_type_parameters" => lower_list_with_commas(node, ctx),
        "value_argument" => lower_value_argument(node, ctx),
        "function_type" => lower_function_type(node, ctx),
        "not_nullable_type" => lower_not_nullable_type(node, ctx),
        "type_parameters" | "type_arguments" => lower_type_list(node, ctx),
        "type_constraints" => lower_type_constraints(node, ctx),
        "type_constraint" | "type_parameter" => lower_type_parameter(node, ctx),
        "delegation_specifier" | "explicit_delegation" => lower_delegation_specifier(node, ctx),
        "collection_literal" => lower_collection_literal(node, ctx),
        "if_expression" => lower_if_expression(node, ctx),
        "try_expression" => lower_try_expression(node, ctx),
        "catch_block" => lower_catch_block(node, ctx),
        "finally_block" => lower_finally_block(node, ctx),
        "control_structure_body" => lower_control_structure_body(node, ctx),
        "type_test" | "range_test" => lower_type_or_range_test(node, ctx),
        "prefix_expression" => lower_prefix_expression(node, ctx),
        "postfix_expression" => lower_postfix_expression(node, ctx),
        "string_literal" | "multiline_string_literal" => lower_string_literal(node, ctx),
        "assignment" => lower_assignment(node, ctx),
        "directly_assignable_expression" => lower_directly_assignable_expression(node, ctx),
        "while_statement" => lower_while_statement(node, ctx),
        "do_while_statement" => lower_do_while_statement(node, ctx),
        "for_statement" => lower_for_statement(node, ctx),
        "shebang_line" => Doc::text(ctx.node_text(&node)),
        "comment" | "line_comment" | "multiline_comment" => lower_comment(node, ctx),
        "annotation" => lower_annotation(node, ctx),
        _ => return None,
    };
    Some(doc)
}

pub fn lower_node<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    if let Some((target_id, replacement)) = ctx.leaf_override
        && node.id() == target_id
    {
        return lower_node(replacement, ctx);
    }

    if ctx.leaf_override.is_none()
        && let Some(elvis) = find_leftmost_unparenthesized_elvis(node)
        && elvis.id() != node.id()
        && is_spine_root(&node)
    {
        return lower_unparenthesized_elvis_chain(node, elvis, ctx);
    }

    if let Some(doc) = lower_declaration_node(node, ctx) {
        return doc;
    }

    if let Some(doc) = lower_expression_node(node, ctx) {
        return doc;
    }

    if node.kind() == ";" {
        if ctx.is_redundant_semicolon(&node) {
            Doc::empty()
        } else {
            Doc::text(";")
        }
    } else if node.child_count() == 0 || node.kind() == "character_literal" {
        Doc::text(ctx.node_text(&node))
    } else {
        lower_composite_node(node, ctx)
    }
}

fn lower_composite_node<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_node(child, ctx));
    }
    Doc::Concat(docs)
}

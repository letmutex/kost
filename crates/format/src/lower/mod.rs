pub mod annotation;
pub mod assignment;
pub mod backing_field;
pub mod binary;
pub mod binary_chain;
pub mod block;
pub mod builders;
pub mod chain;
pub mod chain_call;
pub mod chain_format;
pub mod chain_step;
pub mod class_body;
pub mod constructors;
pub mod context;
pub mod context_receiver;
pub mod control;
pub mod control_utils;
pub mod cst_norm;
pub mod decl;
pub mod destructuring;
pub mod func;
pub mod imports;
pub mod lambda;
pub mod lambda_params;
pub mod loops;
pub mod params;
pub mod property;
pub mod source_file;
pub mod strings;
pub mod try_catch;
pub mod types;
pub mod unary;
pub mod when_expr;

use tree_sitter::Node;

use crate::doc::Doc;
use crate::lower::chain::{flatten_spine, format_chain_spine, is_spine_root};
pub use crate::lower::context::LowerContext;
use crate::verbatim::lower_verbatim;

fn lower_declaration_node<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Option<Doc<'a>> {
    let doc = match node.kind() {
        "class_declaration"
        | "interface_declaration"
        | "companion_object"
        | "object_declaration"
        | "object_literal" => decl::lower_class_declaration(node, ctx, &lower_node),
        "class_body" | "enum_class_body" => class_body::lower_class_body(node, ctx, &lower_node),
        "enum_entry" => class_body::lower_enum_entry(node, ctx, &lower_node),
        "function_declaration" | "anonymous_function" => {
            func::lower_function_declaration(node, ctx, &lower_node)
        }
        "getter" | "setter" => func::lower_accessor(node, ctx, &lower_node),
        "property_declaration" => property::lower_property_declaration(node, ctx, &lower_node),
        "variable_declaration" => property::lower_variable_declaration(node, ctx, &lower_node),
        "parameter" | "class_parameter" | "parameter_with_optional_type" => {
            params::lower_parameter(node, ctx, &lower_node)
        }
        "multi_variable_declaration" => {
            property::lower_multi_variable_declaration(node, ctx, &lower_node)
        }
        "destructuring_declaration" => {
            assignment::lower_destructuring_declaration(node, ctx, &lower_node)
        }
        "backing_field" => backing_field::lower_backing_field(node, ctx, &lower_node),
        "primary_constructor" => constructors::lower_primary_constructor(node, ctx, &lower_node),
        "secondary_constructor" => {
            constructors::lower_secondary_constructor(node, ctx, &lower_node)
        }
        "constructor_delegation_call" => {
            constructors::lower_constructor_delegation_call(node, ctx, &lower_node)
        }
        "anonymous_initializer" => class_body::lower_anonymous_initializer(node, ctx, &lower_node),
        "type_alias" | "typealias" => types::lower_type_alias(node, ctx, &lower_node),
        "type_parameter" | "type_constraint" => types::lower_type_parameter(node, ctx, &lower_node),
        "delegation_specifier" | "explicit_delegation" => {
            types::lower_delegation_specifier(node, ctx, &lower_node)
        }
        "modifiers"
        | "type_modifiers"
        | "parameter_modifiers"
        | "type_parameter_modifiers"
        | "type_projection_modifiers" => annotation::lower_modifiers(node, ctx, &lower_node),
        "file_annotation" => annotation::lower_file_annotation(node, ctx, &lower_node),
        "annotation" => annotation::lower_annotation(node, ctx, &lower_node),
        "context_receiver" | "context_parameters" => {
            context_receiver::lower_context_receiver(node, ctx, &lower_node)
        }
        _ => return None,
    };
    Some(doc)
}

fn lower_expression_node<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Option<Doc<'a>> {
    let doc = match node.kind() {
        "assignment" => assignment::lower_assignment(node, ctx, &lower_node),
        "type_parameters" | "type_arguments" => types::lower_type_list(node, ctx, &lower_node),
        "type_constraints" => types::lower_type_constraints(node, ctx, &lower_node),
        "user_type" => types::lower_user_type(node, ctx, &lower_node),
        "_simple_user_type" => types::lower_simple_user_type(node, ctx, &lower_node),
        "type_projection" => types::lower_type_projection(node, ctx, &lower_node),
        "nullable_type" => types::lower_nullable_type(node, ctx, &lower_node),
        "function_type" => types::lower_function_type(node, ctx, &lower_node),
        "not_nullable_type" => types::lower_not_nullable_type(node, ctx, &lower_node),
        "lambda_literal" => lambda::lower_lambda_literal(node, ctx, &lower_node),
        "annotated_lambda" => lambda::lower_annotated_lambda(node, ctx, &lower_node),
        "lambda_parameters" => lambda::lower_lambda_parameters(node, ctx, &lower_node).0,
        "if_expression" => control::lower_if_expression(node, ctx, &lower_node),
        "when_expression" => when_expr::lower_when_expression(node, ctx, &lower_node),
        "when_entry" => when_expr::lower_when_entry(node, ctx, &lower_node),
        "when_subject" => when_expr::lower_when_subject(node, ctx, &lower_node),
        "guard_condition" => when_expr::lower_guard_condition(node, ctx, &lower_node),
        "try_expression" => try_catch::lower_try_expression(node, ctx, &lower_node),
        "catch_block" => try_catch::lower_catch_block(node, ctx, &lower_node),
        "finally_block" => try_catch::lower_finally_block(node, ctx, &lower_node),
        "for_statement" => loops::lower_for_statement(node, ctx, &lower_node),
        "while_statement" => loops::lower_while_statement(node, ctx, &lower_node),
        "do_while_statement" => loops::lower_do_while_statement(node, ctx, &lower_node),
        "jump_expression" => control::lower_jump_expression(node, ctx, &lower_node),
        "binary_expression"
        | "elvis_expression"
        | "infix_expression"
        | "range_expression"
        | "additive_expression"
        | "multiplicative_expression"
        | "comparison_expression"
        | "equality_expression"
        | "conjunction_expression"
        | "disjunction_expression" => binary::lower_binary_expression(node, ctx, &lower_node),
        "prefix_expression" => unary::lower_prefix_expression(node, ctx, &lower_node),
        "postfix_expression" => unary::lower_postfix_expression(node, ctx, &lower_node),
        "as_expression" | "check_expression" => {
            binary::lower_as_or_check_expression(node, ctx, &lower_node)
        }
        "callable_reference" => types::lower_callable_reference(node, ctx, &lower_node),
        "value_argument" => params::lower_value_argument(node, ctx, &lower_node),
        "string_literal" | "multiline_string_literal" => strings::lower_string_literal(node, ctx),
        "annotated_expression" => unary::lower_prefix_expression(node, ctx, &lower_node),
        "type_test" | "range_test" => control::lower_type_or_range_test(node, ctx, &lower_node),
        "shebang_line" => Doc::text(ctx.node_text(&node)),
        "block" | "function_body" | "statements" | "control_structure_body" => {
            block::lower_block_node(node, ctx, &lower_node)
        }
        _ => return None,
    };
    Some(doc)
}

pub fn lower_node<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let kind = node.kind();

    if kind == ";" {
        return if ctx.is_redundant_semicolon(&node) {
            Doc::empty()
        } else {
            Doc::text(";")
        };
    }

    if kind == "line_comment" {
        return Doc::line_comment(ctx.node_text(&node), false, false);
    }
    if kind == "comment" || kind == "multiline_comment" {
        let text = ctx.node_text(&node);
        if text.starts_with("/**") {
            return crate::rules::kdoc::format_kdoc(text, ctx.options.max_width);
        }
        return Doc::text(text);
    }

    if is_spine_root(&node) {
        let parent_is_spine = node
            .parent()
            .is_some_and(|p| is_spine_root(&p) && p.child(0).is_some_and(|c| c.id() == node.id()));
        if !parent_is_spine {
            let (receiver, segments) = flatten_spine(node, ctx);
            if receiver.id() != node.id() || !segments.is_empty() {
                return format_chain_spine(receiver, segments, ctx, &lower_node);
            }
        }
    }

    match kind {
        "source_file" => source_file::lower_source_file(node, ctx, &lower_node),
        "package_header" => imports::lower_package_header(node, ctx, &lower_node),
        "import_list" => imports::lower_import_list(node, ctx, &lower_node),
        "function_value_parameters"
        | "function_type_parameters"
        | "class_parameters"
        | "value_arguments"
        | "indexing_suffix"
        | "collection_literal"
        | "type_parameters"
        | "type_arguments" => params::lower_delimited_list_node(node, ctx, &lower_node),
        "parenthesized_expression" | "parenthesized_type" | "parenthesized_user_type" => {
            lower_parenthesized(node, ctx)
        }
        "constructor_invocation"
        | "unescaped_annotation"
        | "when_condition"
        | "directly_assignable_expression"
        | "delegation_specifiers"
        | "interpolated_expression"
        | "spread_expression" => lower_transparent_wrapper(node, ctx),
        _ => {
            if let Some(doc) = lower_declaration_node(node, ctx) {
                doc
            } else if let Some(doc) = lower_expression_node(node, ctx) {
                doc
            } else if node.child_count() == 0 || kind == "character_literal" {
                Doc::text(ctx.node_text(&node))
            } else {
                lower_verbatim(node, ctx.source)
            }
        }
    }
}

fn lower_transparent_wrapper<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        docs.push(lower_node(child, ctx));
    }
    Doc::concat(docs)
}

fn lower_parenthesized<'a>(node: Node<'a>, ctx: &LowerContext<'a, '_>) -> Doc<'a> {
    let mut docs = Vec::new();
    docs.push(Doc::text("("));
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let kind = child.kind();
        if kind != "(" && kind != ")" {
            docs.push(lower_node(child, ctx));
        }
    }
    docs.push(Doc::text(")"));
    Doc::concat(docs)
}

mod runner;

use runner::{SkippedInput, run_test_group};

fn assert_test_group(sub_dir: &str, warnings: &[SkippedInput]) {
    let result = run_test_group(sub_dir, warnings);
    match result {
        Ok(summary) => {
            assert!(summary.total > 0, "No test fixtures found in '{sub_dir}'");
        }
        Err(failures) => {
            for failure in &failures {
                eprintln!("FAIL: {} ({})", failure.input_path.display(), failure.phase);
                eprintln!("=== EXPECTED ===\n{}", failure.expected);
                eprintln!("=== ACTUAL ===\n{}", failure.actual);
            }
            let first_failure = &failures[0];
            panic!(
                "{} fixtures failed in '{sub_dir}'. First failure:\n{first_failure}",
                failures.len()
            );
        }
    }
}

macro_rules! test_case_groups {
    ($($fn_name:ident: $group:literal),* $(,)?) => {
        $(
            #[test]
            fn $fn_name() {
                assert_test_group($group, &[]);
            }
        )*
    };
}

test_case_groups! {
    test_no_semi_cases: "no_semi",
    test_long_function_cases: "long_function",
    test_too_many_parameters_cases: "too_many_parameters",
    test_deep_indent_cases: "deep_indent",
    test_wildcard_import_cases: "wildcard_import",
    test_suppression_cases: "suppression",
    test_filename_cases: "filename",
    test_package_name_cases: "package_name",
    test_blank_line_package_cases: "blank_line_package",
    test_blank_line_imports_cases: "blank_line_imports",
    test_blank_line_file_annotation_cases: "blank_line_file_annotation",
    test_import_ordering_cases: "import_ordering",
    test_no_wildcard_imports_cases: "no_wildcard_imports",
    test_no_unused_imports_cases: "no_unused_imports",
    test_blank_line_declaration_cases: "blank_line_declaration",
    test_spacing_annotated_cases: "spacing_annotated",
    test_spacing_comments_cases: "spacing_comments",
    test_no_empty_file_cases: "no_empty_file",
    test_annotation_spacing_cases: "annotation_spacing",
    test_colon_spacing_cases: "colon_spacing",
    test_comma_spacing_cases: "comma_spacing",
    test_curly_spacing_cases: "curly_spacing",
    test_dot_spacing_cases: "dot_spacing",
    test_double_colon_spacing_cases: "double_colon_spacing",
    test_keyword_spacing_cases: "keyword_spacing",
    test_op_spacing_cases: "op_spacing",
    test_paren_spacing_cases: "paren_spacing",
    test_range_spacing_cases: "range_spacing",
    test_square_brackets_spacing_cases: "square_brackets_spacing",
    test_unary_op_spacing_cases: "unary_op_spacing",
    test_angle_bracket_spacing_cases: "angle_bracket_spacing",
    test_function_name_spacing_cases: "function_name_spacing",
    test_fun_keyword_spacing_cases: "fun_keyword_spacing",
    test_function_return_type_spacing_cases: "function_return_type_spacing",
    test_function_start_of_body_spacing_cases: "function_start_of_body_spacing",
    test_function_type_modifier_spacing_cases: "function_type_modifier_spacing",
    test_function_type_reference_spacing_cases: "function_type_reference_spacing",
    test_modifier_list_spacing_cases: "modifier_list_spacing",
    test_modifier_order_cases: "modifier_order",
    test_nullable_type_spacing_cases: "nullable_type_spacing",
    test_parameter_list_spacing_cases: "parameter_list_spacing",
    test_then_spacing_cases: "then_spacing",
    test_try_catch_finally_spacing_cases: "try_catch_finally_spacing",
    test_type_argument_list_spacing_cases: "type_argument_list_spacing",
    test_type_parameter_list_spacing_cases: "type_parameter_list_spacing",
    test_no_multi_spaces_cases: "no_multi_spaces",
    test_no_trailing_spaces_cases: "no_trailing_spaces",
    test_argument_list_wrapping_cases: "argument_list_wrapping",
    test_binary_expression_wrapping_cases: "binary_expression_wrapping",
    test_call_expression_wrapping_cases: "call_expression_wrapping",
    test_chain_method_continuation_cases: "chain_method_continuation",
    test_chain_wrapping_cases: "chain_wrapping",
    test_class_signature_cases: "class_signature",
    test_context_parameter_list_wrapping_cases: "context_parameter_list_wrapping",
    test_context_receiver_wrapping_cases: "context_receiver_wrapping",
    test_enum_wrapping_cases: "enum_wrapping",
    test_expression_operand_wrapping_cases: "expression_operand_wrapping",
    test_function_expression_body_cases: "function_expression_body",
    test_function_signature_cases: "function_signature",
    test_if_else_wrapping_cases: "if_else_wrapping",
    test_multiline_expression_wrapping_cases: "multiline_expression_wrapping",
    test_multiline_if_else_cases: "multiline_if_else",
    test_multiline_loop_cases: "multiline_loop",
    test_no_line_break_after_else_cases: "no_line_break_after_else",
    test_no_line_break_before_assignment_cases: "no_line_break_before_assignment",
    test_parameter_list_wrapping_cases: "parameter_list_wrapping",
    test_parameter_wrapping_cases: "parameter_wrapping",
    test_property_wrapping_cases: "property_wrapping",
    test_statement_wrapping_cases: "statement_wrapping",
    test_wrapping_cases: "wrapping",
    test_blank_line_between_when_conditions_cases: "blank_line_between_when_conditions",
    test_no_blank_line_at_start_of_file_cases: "no_blank_line_at_start_of_file",
    test_no_blank_line_before_rbrace_cases: "no_blank_line_before_rbrace",
    test_no_blank_line_in_list_cases: "no_blank_line_in_list",
    test_no_blank_lines_in_chained_method_calls_cases: "no_blank_lines_in_chained_method_calls",
    test_no_consecutive_blank_lines_cases: "no_consecutive_blank_lines",
    test_no_empty_class_body_cases: "no_empty_class_body",
    test_no_empty_first_line_in_class_body_cases: "no_empty_first_line_in_class_body",
    test_no_empty_first_line_in_method_block_cases: "no_empty_first_line_in_method_block",
    test_no_single_line_block_comment_cases: "no_single_line_block_comment",
    test_no_unit_return_cases: "no_unit_return",
    test_unnecessary_parentheses_before_trailing_lambda_cases: "unnecessary_parentheses_before_trailing_lambda",
    test_trailing_comma_on_call_site_cases: "trailing_comma_on_call_site",
    test_trailing_comma_on_declaration_site_cases: "trailing_comma_on_declaration_site",
    test_kdoc_wrapping_cases: "kdoc_wrapping",
    test_string_template_cases: "string_template",
    test_function_literal_cases: "function_literal",
    test_if_else_bracing_cases: "if_else_bracing",
    test_lambda_return_cases: "lambda_return",
    test_mixed_condition_operators_cases: "mixed_condition_operators",
    test_string_template_indent_cases: "string_template_indent",
    test_when_entry_bracing_cases: "when_entry_bracing",
    test_block_comment_initial_star_alignment_cases: "block_comment_initial_star_alignment",
    test_comment_spacing_cases: "comment_spacing",
    test_comment_wrapping_cases: "comment_wrapping",
    test_kdoc_cases: "kdoc",
    test_kdoc_delimiter_cases: "kdoc_delimiter",
    test_type_argument_comment_cases: "type_argument_comment",
    test_type_parameter_comment_cases: "type_parameter_comment",
    test_value_argument_comment_cases: "value_argument_comment",
    test_no_consecutive_comments_cases: "no_consecutive_comments",
    test_class_naming_cases: "class_naming",
    test_function_naming_cases: "function_naming",
    test_property_naming_cases: "property_naming",
    test_backing_property_naming_cases: "backing_property_naming",
    test_enum_entry_name_case_cases: "enum_entry_name_case",
    test_annotation_cases: "annotation",
    test_value_parameter_comment_cases: "value_parameter_comment",
    test_indent_cases: "indent",
    test_max_line_length_cases: "max_line_length",
    test_final_newline_cases: "final_newline",
    test_class_name_length_cases: "class_name_length",
    test_function_name_length_cases: "function_name_length",
    test_variable_name_length_cases: "variable_name_length",
    test_file_length_cases: "file_length",
    test_too_many_functions_cases: "too_many_functions",
    test_string_operators_cases: "string_operators",
}

//! Factory instantiation for built-in compiled AST rules.

use crate::rule::AstRule;

use super::angle_bracket_spacing::AngleBracketSpacing;
use super::annotation::Annotation;
use super::annotation_spacing::AnnotationSpacing;
use super::argument_list_wrapping::ArgumentListWrapping;
use super::backing_property_naming::BackingPropertyNaming;
use super::binary_expression_wrapping::BinaryExpressionWrapping;
use super::blank_line_between_when_conditions::BlankLineBetweenWhenConditions;
use super::blank_line_declaration::BlankLineBeforeDeclaration;
use super::blank_line_file_annotation::BlankLineBeforeFileAnnotation;
use super::blank_line_imports::BlankLineBeforeImports;
use super::blank_line_package::BlankLineBeforePackage;
use super::block_comment_initial_star_alignment::BlockCommentInitialStarAlignment;
use super::call_expression_wrapping::CallExpressionWrapping;
use super::chain_method_continuation::ChainMethodContinuation;
use super::chain_wrapping::ChainWrapping;
use super::class_naming::ClassNaming;
use super::class_signature::ClassSignature;
use super::colon_spacing::ColonSpacing;
use super::comma_spacing::CommaSpacing;
use super::comment_spacing::CommentSpacing;
use super::comment_wrapping::CommentWrapping;
use super::context_parameter_list_wrapping::ContextParameterListWrapping;
use super::context_receiver_wrapping::ContextReceiverWrapping;
use super::curly_spacing::CurlySpacing;
use super::dot_spacing::DotSpacing;
use super::double_colon_spacing::DoubleColonSpacing;
use super::enum_entry_name_case::EnumEntryNameCase;
use super::enum_wrapping::EnumWrapping;
use super::expression_operand_wrapping::ExpressionOperandWrapping;
use super::filename::Filename;
use super::final_newline::FinalNewline;
use super::fun_keyword_spacing::FunKeywordSpacing;
use super::function_expression_body::FunctionExpressionBody;
use super::function_literal::FunctionLiteral;
use super::function_name_spacing::FunctionNameSpacing;
use super::function_naming::FunctionNaming;
use super::function_return_type_spacing::FunctionReturnTypeSpacing;
use super::function_signature::FunctionSignature;
use super::function_start_of_body_spacing::FunctionStartOfBodySpacing;
use super::function_type_modifier_spacing::FunctionTypeModifierSpacing;
use super::function_type_reference_spacing::FunctionTypeReferenceSpacing;
use super::if_else_bracing::IfElseBracing;
use super::if_else_wrapping::IfElseWrapping;
use super::import_ordering::ImportOrdering;
use super::indent::Indent;
use super::kdoc::Kdoc;
use super::kdoc_delimiter::KdocDelimiter;
use super::kdoc_wrapping::KdocWrapping;
use super::keyword_spacing::KeywordSpacing;
use super::lambda_return::LambdaReturn;
use super::max_line_length::MaxLineLength;
use super::mixed_condition_operators::MixedConditionOperators;
use super::modifier_list_spacing::ModifierListSpacing;
use super::modifier_order::ModifierOrder;
use super::multiline_expression_wrapping::MultilineExpressionWrapping;
use super::multiline_if_else::MultilineIfElse;
use super::multiline_loop::MultilineLoop;
use super::no_blank_line_at_start_of_file::NoBlankLineAtStartOfFile;
use super::no_blank_line_before_rbrace::NoBlankLineBeforeRbrace;
use super::no_blank_line_in_list::NoBlankLineInList;
use super::no_blank_lines_in_chained_method_calls::NoBlankLinesInChainedMethodCalls;
use super::no_consecutive_blank_lines::NoConsecutiveBlankLines;
use super::no_consecutive_comments::NoConsecutiveComments;
use super::no_empty_class_body::NoEmptyClassBody;
use super::no_empty_file::NoEmptyFile;
use super::no_empty_first_line_in_class_body::NoEmptyFirstLineInClassBody;
use super::no_empty_first_line_in_method_block::NoEmptyFirstLineInMethodBlock;
use super::no_fully_qualified_type_names::NoFullyQualifiedTypeNames;
use super::no_line_break_after_else::NoLineBreakAfterElse;
use super::no_line_break_before_assignment::NoLineBreakBeforeAssignment;
use super::no_multi_spaces::NoMultiSpaces;
use super::no_semi::NoSemi;
use super::no_single_line_block_comment::NoSingleLineBlockComment;
use super::no_trailing_spaces::NoTrailingSpaces;
use super::no_unit_return::NoUnitReturn;
use super::no_unused_imports::NoUnusedImports;
use super::no_wildcard_imports::NoWildcardImports;
use super::nullable_type_spacing::NullableTypeSpacing;
use super::op_spacing::OpSpacing;
use super::package_name::PackageName;
use super::parameter_list_spacing::ParameterListSpacing;
use super::parameter_list_wrapping::ParameterListWrapping;
use super::parameter_wrapping::ParameterWrapping;
use super::paren_spacing::ParenSpacing;
use super::property_naming::PropertyNaming;
use super::property_wrapping::PropertyWrapping;
use super::range_spacing::RangeSpacing;
use super::spacing_annotated::SpacingBetweenDeclarationsWithAnnotations;
use super::spacing_comments::SpacingBetweenDeclarationsWithComments;
use super::square_brackets_spacing::SquareBracketsSpacing;
use super::statement_wrapping::StatementWrapping;
use super::string_template::StringTemplate;
use super::string_template_indent::StringTemplateIndent;
use super::then_spacing::ThenSpacing;
use super::trailing_comma_on_call_site::TrailingCommaOnCallSite;
use super::trailing_comma_on_declaration_site::TrailingCommaOnDeclarationSite;
use super::try_catch_finally_spacing::TryCatchFinallySpacing;
use super::type_argument_comment::TypeArgumentComment;
use super::type_argument_list_spacing::TypeArgumentListSpacing;
use super::type_parameter_comment::TypeParameterComment;
use super::type_parameter_list_spacing::TypeParameterListSpacing;
use super::unary_op_spacing::UnaryOpSpacing;
use super::unnecessary_parentheses_before_trailing_lambda::UnnecessaryParenthesesBeforeTrailingLambda;
use super::value_argument_comment::ValueArgumentComment;
use super::value_parameter_comment::ValueParameterComment;
use super::when_entry_bracing::WhenEntryBracing;
use super::wrapping::Wrapping;

/// Creates an instance of a standard AST rule by its local canonical name.
pub fn create_standard_ast_rule(name: &str) -> Option<Box<dyn AstRule>> {
    create_spacing_and_file_rule(name)
        .or_else(|| create_wrapping_and_blank_rule(name))
        .or_else(|| create_cleanup_and_trailing_rule(name))
        .or_else(|| create_comment_rule(name))
        .or_else(|| create_naming_and_formatting_rule(name))
}

fn create_spacing_and_file_rule(name: &str) -> Option<Box<dyn AstRule>> {
    match name {
        "filename" => Some(Box::new(Filename)),
        "package-name" => Some(Box::new(PackageName)),
        "blank-line-before-package" => Some(Box::new(BlankLineBeforePackage)),
        "blank-line-before-imports" => Some(Box::new(BlankLineBeforeImports)),
        "blank-line-before-file-annotation" => Some(Box::new(BlankLineBeforeFileAnnotation)),
        "import-ordering" => Some(Box::new(ImportOrdering)),
        "no-wildcard-imports" => Some(Box::new(NoWildcardImports)),
        "no-unused-imports" => Some(Box::new(NoUnusedImports)),
        "blank-line-before-declaration" => Some(Box::new(BlankLineBeforeDeclaration)),
        "spacing-between-declarations-with-annotations" => {
            Some(Box::new(SpacingBetweenDeclarationsWithAnnotations))
        }
        "spacing-between-declarations-with-comments" => {
            Some(Box::new(SpacingBetweenDeclarationsWithComments))
        }
        "no-empty-file" => Some(Box::new(NoEmptyFile)),
        "annotation-spacing" => Some(Box::new(AnnotationSpacing)),
        "colon-spacing" => Some(Box::new(ColonSpacing)),
        "comma-spacing" => Some(Box::new(CommaSpacing)),
        "curly-spacing" => Some(Box::new(CurlySpacing)),
        "dot-spacing" => Some(Box::new(DotSpacing)),
        "double-colon-spacing" => Some(Box::new(DoubleColonSpacing)),
        "keyword-spacing" => Some(Box::new(KeywordSpacing)),
        "op-spacing" => Some(Box::new(OpSpacing)),
        "paren-spacing" => Some(Box::new(ParenSpacing)),
        "range-spacing" => Some(Box::new(RangeSpacing)),
        "square-brackets-spacing" => Some(Box::new(SquareBracketsSpacing)),
        "unary-op-spacing" => Some(Box::new(UnaryOpSpacing)),
        "spacing-around-angle-brackets" => Some(Box::new(AngleBracketSpacing)),
        "spacing-between-function-name-and-opening-parenthesis" => {
            Some(Box::new(FunctionNameSpacing))
        }
        "fun-keyword-spacing" => Some(Box::new(FunKeywordSpacing)),
        "function-return-type-spacing" => Some(Box::new(FunctionReturnTypeSpacing)),
        "function-start-of-body-spacing" => Some(Box::new(FunctionStartOfBodySpacing)),
        "function-type-modifier-spacing" => Some(Box::new(FunctionTypeModifierSpacing)),
        "function-type-reference-spacing" => Some(Box::new(FunctionTypeReferenceSpacing)),
        "modifier-list-spacing" => Some(Box::new(ModifierListSpacing)),
        "modifier-order" => Some(Box::new(ModifierOrder)),
        "nullable-type-spacing" => Some(Box::new(NullableTypeSpacing)),
        "parameter-list-spacing" => Some(Box::new(ParameterListSpacing)),
        "then-spacing" => Some(Box::new(ThenSpacing)),
        "try-catch-finally-spacing" => Some(Box::new(TryCatchFinallySpacing)),
        "type-argument-list-spacing" => Some(Box::new(TypeArgumentListSpacing)),
        "type-parameter-list-spacing" => Some(Box::new(TypeParameterListSpacing)),
        "no-multi-spaces" => Some(Box::new(NoMultiSpaces)),
        "no-semi" => Some(Box::new(NoSemi)),
        _ => None,
    }
}

fn create_wrapping_and_blank_rule(name: &str) -> Option<Box<dyn AstRule>> {
    match name {
        "no-trailing-spaces" => Some(Box::new(NoTrailingSpaces)),
        "argument-list-wrapping" => Some(Box::new(ArgumentListWrapping)),
        "binary-expression-wrapping" => Some(Box::new(BinaryExpressionWrapping)),
        "call-expression-wrapping" => Some(Box::new(CallExpressionWrapping)),
        "chain-method-continuation" => Some(Box::new(ChainMethodContinuation)),
        "chain-wrapping" => Some(Box::new(ChainWrapping)),
        "class-signature" => Some(Box::new(ClassSignature)),
        "context-parameter-list-wrapping" => Some(Box::new(ContextParameterListWrapping)),
        "context-receiver-wrapping" => Some(Box::new(ContextReceiverWrapping)),
        "enum-wrapping" => Some(Box::new(EnumWrapping)),
        "expression-operand-wrapping" => Some(Box::new(ExpressionOperandWrapping)),
        "function-expression-body" => Some(Box::new(FunctionExpressionBody)),
        "function-signature" => Some(Box::new(FunctionSignature)),
        "if-else-wrapping" => Some(Box::new(IfElseWrapping)),
        "multiline-expression-wrapping" => Some(Box::new(MultilineExpressionWrapping)),
        "multiline-if-else" => Some(Box::new(MultilineIfElse)),
        "multiline-loop" => Some(Box::new(MultilineLoop)),
        "no-line-break-after-else" => Some(Box::new(NoLineBreakAfterElse)),
        "no-line-break-before-assignment" => Some(Box::new(NoLineBreakBeforeAssignment)),
        "parameter-list-wrapping" => Some(Box::new(ParameterListWrapping)),
        "parameter-wrapping" => Some(Box::new(ParameterWrapping)),
        "property-wrapping" => Some(Box::new(PropertyWrapping)),
        "statement-wrapping" => Some(Box::new(StatementWrapping)),
        "wrapping" => Some(Box::new(Wrapping)),
        "blank-line-between-when-conditions" => Some(Box::new(BlankLineBetweenWhenConditions)),
        "no-blank-line-at-start-of-file" => Some(Box::new(NoBlankLineAtStartOfFile)),
        "no-blank-line-before-rbrace" => Some(Box::new(NoBlankLineBeforeRbrace)),
        "no-blank-line-in-list" => Some(Box::new(NoBlankLineInList)),
        "no-blank-lines-in-chained-method-calls" => {
            Some(Box::new(NoBlankLinesInChainedMethodCalls))
        }
        "no-consecutive-blank-lines" => Some(Box::new(NoConsecutiveBlankLines)),
        _ => None,
    }
}

fn create_cleanup_and_trailing_rule(name: &str) -> Option<Box<dyn AstRule>> {
    match name {
        "no-empty-class-body" => Some(Box::new(NoEmptyClassBody)),
        "no-empty-first-line-in-class-body" => Some(Box::new(NoEmptyFirstLineInClassBody)),
        "no-empty-first-line-in-method-block" => Some(Box::new(NoEmptyFirstLineInMethodBlock)),
        "no-fully-qualified-type-names" => Some(Box::new(NoFullyQualifiedTypeNames)),
        "no-single-line-block-comment" => Some(Box::new(NoSingleLineBlockComment)),
        "no-unit-return" => Some(Box::new(NoUnitReturn)),
        "unnecessary-parentheses-before-trailing-lambda" => {
            Some(Box::new(UnnecessaryParenthesesBeforeTrailingLambda))
        }
        "trailing-comma-on-call-site" => Some(Box::new(TrailingCommaOnCallSite)),
        "trailing-comma-on-declaration-site" => Some(Box::new(TrailingCommaOnDeclarationSite)),
        "kdoc-wrapping" => Some(Box::new(KdocWrapping)),
        "string-template" => Some(Box::new(StringTemplate)),
        "function-literal" => Some(Box::new(FunctionLiteral)),
        "if-else-bracing" => Some(Box::new(IfElseBracing)),
        "lambda-return" => Some(Box::new(LambdaReturn)),
        "mixed-condition-operators" => Some(Box::new(MixedConditionOperators)),
        "string-template-indent" => Some(Box::new(StringTemplateIndent)),
        "when-entry-bracing" => Some(Box::new(WhenEntryBracing)),
        _ => None,
    }
}

fn create_comment_rule(name: &str) -> Option<Box<dyn AstRule>> {
    match name {
        "block-comment-initial-star-alignment" => Some(Box::new(BlockCommentInitialStarAlignment)),
        "comment-spacing" => Some(Box::new(CommentSpacing)),
        "comment-wrapping" => Some(Box::new(CommentWrapping)),
        "kdoc" => Some(Box::new(Kdoc)),
        "kdoc-delimiter" => Some(Box::new(KdocDelimiter)),
        "type-argument-comment" => Some(Box::new(TypeArgumentComment)),
        "type-parameter-comment" => Some(Box::new(TypeParameterComment)),
        "value-argument-comment" => Some(Box::new(ValueArgumentComment)),
        _ => None,
    }
}

fn create_naming_and_formatting_rule(name: &str) -> Option<Box<dyn AstRule>> {
    match name {
        "no-consecutive-comments" => Some(Box::new(NoConsecutiveComments)),
        "class-naming" => Some(Box::new(ClassNaming)),
        "function-naming" => Some(Box::new(FunctionNaming)),
        "property-naming" => Some(Box::new(PropertyNaming)),
        "backing-property-naming" => Some(Box::new(BackingPropertyNaming)),
        "enum-entry-name-case" => Some(Box::new(EnumEntryNameCase)),
        "annotation" => Some(Box::new(Annotation)),
        "value-parameter-comment" => Some(Box::new(ValueParameterComment)),
        "indent" => Some(Box::new(Indent)),
        "max-line-length" => Some(Box::new(MaxLineLength)),
        "final-newline" => Some(Box::new(FinalNewline)),
        _ => None,
    }
}

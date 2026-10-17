use lsp::indexer::{DeclarationItem, DeclarationMetadata, WorkspaceIndex};
use lsp::inference::KotlinType;
use lsp::inference::score_overload_candidate;
use tower_lsp::lsp_types::{Range, SymbolKind};

fn make_test_decl(name: &str, sig: &str) -> DeclarationItem {
    DeclarationItem::new(
        name,
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::signature(sig),
    )
}

#[test]
fn test_overload_exact_type_match_bytecode_beats_mismatched_source() {
    let index = WorkspaceIndex::new();
    let decl_bc = make_test_decl("process", "fun process(id: Int): Unit");
    let decl_src = make_test_decl("process", "fun process(id: Int, flag: Boolean): Unit");

    let raw_args = &["10"];
    let inferred = &[KotlinType::parse("Int")];

    let score_bc = score_overload_candidate(&decl_bc, false, raw_args, inferred, &index)
        .expect("Bytecode overload with exact arity and type should be scored");
    let score_src = score_overload_candidate(&decl_src, true, raw_args, inferred, &index)
        .expect("Source overload with partial arity should be scored");

    assert!(
        score_bc > score_src,
        "Exact arity match from bytecode (1 param) must score higher than arity mismatch from source (2 params), but is_source precedence caused source to win"
    );
}

#[test]
fn test_overload_non_vararg_rejects_more_arguments_than_parameters() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("calculate", "fun calculate(a: Int, b: Int): Int");

    // 2 parameters, but 3 arguments (2 positional, 1 named)
    let raw_args = &["1", "2", "b = 3"];
    let inferred = &[
        KotlinType::parse("Int"),
        KotlinType::parse("Int"),
        KotlinType::parse("Int"),
    ];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Non-vararg function with 2 parameters must reject call with 3 arguments (calculate(1, 2, b = 3))"
    );
}

#[test]
fn test_overload_rejects_incompatible_primitive_to_class_argument() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("render", "fun render(widget: CustomWidget): Unit");

    let raw_args = &["\"hello\""];
    let inferred = &[KotlinType::parse("String")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing a String primitive argument to a CustomWidget parameter must be rejected as incompatible"
    );
}

#[test]
fn test_overload_rejects_duplicate_named_arguments() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("configure", "fun configure(width: Int, height: Int): Unit");

    let raw_args = &["width = 10", "width = 20"];
    let inferred = &[KotlinType::parse("Int"), KotlinType::parse("Int")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Duplicate named arguments (width = 10, width = 20) must be rejected"
    );
}

#[test]
fn test_overload_any_argument_scores_higher_for_any_parameter_than_narrow_parameter() {
    let index = WorkspaceIndex::new();
    let decl_any = make_test_decl("handle", "fun handle(x: Any): String");
    let decl_int = make_test_decl("handle", "fun handle(x: Int): String");

    let raw_args = &["item"];
    let inferred = &[KotlinType::parse("kotlin.Any")];

    let score_any = score_overload_candidate(&decl_any, true, raw_args, inferred, &index)
        .expect("Any overload should be candidate");
    let score_int = score_overload_candidate(&decl_int, true, raw_args, inferred, &index)
        .expect("Int overload candidate");

    assert!(
        score_any > score_int,
        "Passing kotlin.Any argument must score higher on Any parameter than on Int parameter, but got score_any={score_any:?} and score_int={score_int:?}"
    );
}

#[test]
fn test_overload_accepts_primitive_argument_for_multi_character_generic_type_parameter() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("cache", "fun <Key> cache(key: Key): Unit");

    let raw_args = &["\"user_123\""];
    let inferred = &[KotlinType::parse("String")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing a String primitive to a generic parameter 'Key' must be accepted, but was rejected by is_primitive_incompatible"
    );
}

#[test]
fn test_overload_accepts_argument_matching_generic_type_upper_bound() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("format", "fun <Key : CharSequence> format(key: Key): Unit");

    let raw_args = &["\"title\""];
    let inferred = &[KotlinType::parse("String")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing String to Key bounded by CharSequence must be accepted"
    );
}

#[test]
fn test_overload_rejects_argument_violating_generic_type_upper_bound() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("calculate", "fun <Key : Number> calculate(key: Key): Unit");

    let raw_args = &["\"not_a_number\""];
    let inferred = &[KotlinType::parse("String")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing String to Key bounded by Number must be rejected due to bound violation"
    );
}

#[test]
fn test_overload_prefers_concrete_parameter_over_generic_parameter() {
    let index = WorkspaceIndex::new();
    let decl_concrete = make_test_decl("render", "fun render(text: String): Unit");
    let decl_generic = make_test_decl("render", "fun <Key> render(text: Key): Unit");

    let raw_args = &["\"hello\""];
    let inferred = &[KotlinType::parse("String")];

    let score_concrete = score_overload_candidate(&decl_concrete, true, raw_args, inferred, &index)
        .expect("Concrete overload should score");
    let score_generic = score_overload_candidate(&decl_generic, true, raw_args, inferred, &index)
        .expect("Generic overload should score");

    assert!(
        score_concrete > score_generic,
        "Concrete parameter String must score higher than generic parameter Key"
    );
}

#[test]
fn test_overload_accepts_primitive_in_parameterized_generic_container() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl(
        "processAll",
        "fun <Element> processAll(items: List<Element>): Unit",
    );

    let raw_args = &["items"];
    let inferred = &[KotlinType::parse("List<String>")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing List<String> to List<Element> with generic parameter Element must be accepted"
    );
}

#[test]
fn test_overload_rejects_incompatible_primitive_in_parameterized_container() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("processAll", "fun processAll(items: List<Int>): Unit");

    let raw_args = &["items"];
    let inferred = &[KotlinType::parse("List<String>")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing List<String> to List<Int> must be rejected as incompatible"
    );
}

#[test]
fn test_overload_rejects_incompatible_type_in_non_trailing_vararg_parameter() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl(
        "format",
        "fun format(vararg values: String, prefix: String = \"\"): String",
    );

    let raw_args = &["\"hello\"", "123", "prefix = \"-\""];
    let inferred = &[
        KotlinType::parse("String"),
        KotlinType::parse("Int"),
        KotlinType::parse("String"),
    ];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing an Int (123) to a non-trailing vararg values: String must be rejected as incompatible"
    );
}

#[test]
fn test_overload_accepts_primitive_argument_for_parameterized_comparable_bound() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("sort", "fun <T : Comparable<T>> sort(item: T): Unit");

    let raw_args = &["\"hello\""];
    let inferred = &[KotlinType::parse("String")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing String primitive to parameter T bounded by Comparable<T> must be accepted"
    );
}

#[test]
fn test_overload_does_not_treat_concrete_receiver_type_argument_as_generic_parameter() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("process", "fun List<String>.process(text: String): Unit");

    let raw_args = &["123"];
    let inferred = &[KotlinType::parse("Int")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing Int to text: String on an extension function List<String> must be rejected, not treated as generic parameter"
    );
}

#[test]
fn test_overload_rejects_nullable_argument_for_non_nullable_parameter() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("process", "fun process(text: String): Unit");

    let raw_args = &["text"];
    let inferred = &[KotlinType::parse("String?")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing nullable String? to non-nullable parameter String must be rejected"
    );
}

#[test]
fn test_overload_accepts_argument_for_variance_projected_generic_container() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("copy", "fun <T> copy(from: List<out T>): Unit");

    let raw_args = &["items"];
    let inferred = &[KotlinType::parse("List<String>")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing List<String> to List<out T> must match generic parameter T with variance"
    );
}

#[test]
fn test_overload_rejects_argument_with_mismatched_parameterized_bound() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("sort", "fun <T : Comparable<Int>> sort(item: T): Unit");

    let raw_args = &["\"text\""];
    let inferred = &[KotlinType::parse("String")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing String (Comparable<String>) to T bounded by Comparable<Int> must be rejected"
    );
}

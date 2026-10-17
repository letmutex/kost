use std::sync::Arc;

use lsp::indexer::{DeclarationItem, DeclarationMetadata, WorkspaceIndex};
use lsp::inference::overload::{clean_variance, satisfies_bound};
use lsp::inference::{GenericParamInfo, KotlinType, score_overload_candidate};
use tower_lsp::lsp_types::{Range, SymbolKind, Url};

fn make_test_decl(name: &str, signature: &str) -> DeclarationItem {
    DeclarationItem::new(
        name,
        SymbolKind::FUNCTION,
        Range::default(),
        Range::default(),
        DeclarationMetadata::signature(signature),
    )
}

#[test]
fn test_overload_prefers_nullable_parameter_for_nullable_argument() {
    let index = WorkspaceIndex::new();
    let decl_non_null = make_test_decl("format", "fun format(s: String): Unit");
    let decl_null = make_test_decl("format", "fun format(s: String?): Unit");

    let raw_args = &["s"];
    let inferred = &[KotlinType::parse("String?")];

    let score_non_null = score_overload_candidate(&decl_non_null, true, raw_args, inferred, &index);
    let score_null = score_overload_candidate(&decl_null, true, raw_args, inferred, &index);

    assert_eq!(
        score_non_null, None,
        "Passing nullable String? to non-nullable format(s: String) must be rejected"
    );
    assert!(
        score_null.is_some(),
        "Passing nullable String? to nullable format(s: String?) must be accepted"
    );
}

#[test]
fn test_overload_rejects_nullable_argument_for_non_nullable_generic_upper_bound() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("compute", "fun <T : Number> compute(x: T): Unit");

    let raw_args = &["x"];
    let inferred = &[KotlinType::parse("Int?")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing nullable Int? to generic parameter T bounded by non-nullable Number must be rejected"
    );
}

#[test]
fn test_overload_rejects_incompatible_nullable_primitive_in_parameterized_container() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("process", "fun process(items: List<Int?>): Unit");

    let raw_args = &["items"];
    let inferred = &[KotlinType::parse("List<Double?>")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing List<Double?> to List<Int?> must be rejected as incompatible"
    );
}

#[test]
fn test_overload_accepts_argument_for_contravariant_projected_container() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("compare", "fun <T> compare(c: Comparable<in T>): Unit");

    let raw_args = &["c"];
    let inferred = &[KotlinType::parse("Comparable<String>")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing Comparable<String> to Comparable<in T> must match contravariant type parameter"
    );
}

#[test]
fn test_overload_rejects_nullable_type_argument_in_parameterized_container_with_non_nullable_element()
 {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("process", "fun process(items: List<String>): Unit");

    let raw_args = &["items"];
    let inferred = &[KotlinType::parse("List<String?>")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing List<String?> to List<String> must be rejected due to element nullability"
    );
}

#[test]
fn test_overload_accepts_nullable_argument_for_nullable_bounded_generic_parameter() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("compute", "fun <T : Number> compute(x: T?): Unit");

    let raw_args = &["x"];
    let inferred = &[KotlinType::parse("Int?")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing Int? to nullable bounded generic parameter T? bounded by Number must be accepted"
    );
}

#[test]
fn test_overload_matches_parameterized_supertype_for_qualified_container_type() {
    let mut index = WorkspaceIndex::new();
    index
        .class_supertypes
        .entry("MyList".to_string())
        .or_default()
        .push("List<String>".to_string());

    let decl = make_test_decl("process", "fun process(items: List<String>): Unit");

    let raw_args = &["items"];
    let inferred = &[KotlinType::parse("com.example.MyList<String>")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing com.example.MyList<String> should match List<String> via MyList supertype"
    );
}

#[test]
fn test_overload_accepts_parameterized_argument_for_unparameterized_bound_or_parameter() {
    let index = WorkspaceIndex::new();
    let decl_raw_param = make_test_decl("process", "fun process(items: List): Unit");

    let raw_args = &["items"];
    let inferred = &[KotlinType::parse("List<String>")];

    let score = score_overload_candidate(&decl_raw_param, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing List<String> to raw parameter List must be accepted"
    );

    let decl_raw_bound = make_test_decl(
        "processGeneric",
        "fun <T : List> processGeneric(items: T): Unit",
    );
    let score_gen = score_overload_candidate(&decl_raw_bound, true, raw_args, inferred, &index);
    assert!(
        score_gen.is_some(),
        "Passing List<String> to T bounded by raw List must be accepted"
    );
}

#[test]
fn test_overload_accepts_parameterized_argument_for_unparameterized_supertype_bound() {
    let mut index = WorkspaceIndex::new();
    index
        .class_supertypes
        .entry("List".to_string())
        .or_default()
        .push("Collection".to_string());

    let decl = make_test_decl("collect", "fun <T : Collection> collect(items: T): Unit");

    let raw_args = &["items"];
    let inferred = &[KotlinType::parse("List<String>")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing List<String> to T bounded by Collection must be accepted via supertype"
    );
}

#[test]
fn test_overload_rejects_null_argument_for_non_nullable_parameter() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("print", "fun print(s: String): Unit");
    let raw_args = &["null"];
    let inferred = &[KotlinType::parse("Nothing?")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing Nothing? (null) to non-nullable parameter String must be rejected"
    );
}

#[test]
fn test_overload_accepts_nullable_argument_for_nullable_bounded_type_parameter() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("compute", "fun <T : Number?> compute(x: T): Unit");
    let raw_args = &["x"];
    let inferred = &[KotlinType::parse("Int?")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing nullable Int? to type parameter T with nullable bound Number? must be accepted"
    );
}

#[test]
fn test_overload_accepts_nullable_argument_for_explicit_any_nullable_bound() {
    let index = WorkspaceIndex::new();
    let decl = make_test_decl("hold", "fun <T : Any?> hold(x: T): Unit");
    let raw_args = &["x"];
    let inferred = &[KotlinType::parse("String?")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert!(
        score.is_some(),
        "Passing nullable String? to type parameter T bounded by Any? must be accepted"
    );
}

#[test]
fn test_clean_variance_with_tabs_or_newlines() {
    assert_eq!(clean_variance("out\tT"), "T");
    assert_eq!(clean_variance("in\nT"), "T");
}

#[test]
fn test_overload_prefers_exact_non_nullable_parameter_over_nullable_parameter_for_non_nullable_argument()
 {
    let index = WorkspaceIndex::new();
    let decl_non_null = make_test_decl("format", "fun format(s: String): Unit");
    let decl_null = make_test_decl("format", "fun format(s: String?): Unit");

    let raw_args = &["s"];
    let inferred = &[KotlinType::parse("String")];

    let score_non_null = score_overload_candidate(&decl_non_null, true, raw_args, inferred, &index)
        .expect("Exact non-nullable candidate should score");
    let score_null = score_overload_candidate(&decl_null, true, raw_args, inferred, &index)
        .expect("Nullable candidate should score");

    assert!(
        score_non_null.type_matches > score_null.type_matches,
        "Exact non-nullable parameter type must score higher than nullable parameter for non-nullable argument"
    );
}

#[test]
fn test_primitive_comparable_bound_rejects_single_letter_concrete_class_when_type_params_declared()
{
    let index = WorkspaceIndex::new();
    let declared_params = vec![GenericParamInfo {
        name: smol_str::SmolStr::new("T"),
        bounds: vec![],
        is_out: false,
    }];

    let satisfies = satisfies_bound("Int", "Comparable<I>", &index, &declared_params);
    assert!(
        !satisfies,
        "Comparable<I> must not treat concrete class I as generic parameter when type_params are declared"
    );
}

#[test]
fn test_overload_rejects_nullable_argument_for_single_letter_concrete_class_parameter() {
    let mut index = WorkspaceIndex::new();
    let class_a = make_test_decl("A", "class A");
    index.name_to_decls.entry("A".into()).or_default().push((
        Arc::new(Url::parse("file:///test/A.kt").unwrap()),
        Arc::new(class_a),
    ));

    let decl = make_test_decl("handle", "fun handle(node: A): Unit");
    let raw_args = &["node"];
    let inferred = &[KotlinType::parse("A?")];

    let score = score_overload_candidate(&decl, true, raw_args, inferred, &index);
    assert_eq!(
        score, None,
        "Passing nullable A? to non-nullable single-letter concrete class parameter A must be rejected"
    );
}

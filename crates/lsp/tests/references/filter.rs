use lsp::features::references::{ReferenceTarget, contains_identifier_token, is_file_candidate};
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Location, Position, Range, Url};

fn create_test_target(uri: Url, name: &str) -> ReferenceTarget {
    ReferenceTarget {
        name: name.to_string(),
        location: Location {
            uri,
            range: Range::new(Position::new(0, 0), Position::new(0, 5)),
        },
        is_local: false,
        local_scope_node_range: None,
        is_private: false,
        is_extension: false,
        is_override: false,
        kind: None,
        container_name: None,
        signature: None,
    }
}

#[test]
fn test_token_boundary_matches_isolated_identifier() {
    assert!(contains_identifier_token("val job: Job = create()", "Job"));
    assert!(contains_identifier_token("fun run(job: Job?)", "Job"));
    assert!(contains_identifier_token("val list: List<Job>", "Job"));
    assert!(contains_identifier_token("Job", "Job"));
    assert!(contains_identifier_token("`Job`", "Job"));
    assert!(contains_identifier_token("(Job)", "Job"));
    assert!(contains_identifier_token("Job!", "Job"));
    assert!(contains_identifier_token("Job.", "Job"));
}

#[test]
fn test_token_boundary_rejects_identifier_substring() {
    assert!(!contains_identifier_token("val job = JobSupport()", "Job"));
    assert!(!contains_identifier_token("class CompletableJob", "Job"));
    assert!(!contains_identifier_token("val myJobInstance = 1", "Job"));
    assert!(!contains_identifier_token("Job_Impl", "Job"));
    assert!(!contains_identifier_token("", "Job"));
    assert!(!contains_identifier_token("Jo", "Job"));
}

#[test]
fn test_token_boundary_matches_interpolated_identifier() {
    assert!(contains_identifier_token("\"Hello, $name!\"", "name"));
    assert!(contains_identifier_token("\"${name}: value\"", "name"));
    assert!(!contains_identifier_token(
        "\"$nameOther finished\"",
        "name"
    ));
}

#[test]
fn test_token_boundary_handles_non_ascii_delimiters() {
    assert!(contains_identifier_token("/* 注释 */ Job", "Job"));
    assert!(!contains_identifier_token("Jobé", "Job"));
    assert!(!contains_identifier_token("éJob", "Job"));
}

#[test]
fn test_candidate_pruning_retains_declaring_file() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/Target.kt").unwrap();
    let source = "class Target";
    index.index_source(file_url.clone(), source);

    let target = create_test_target(file_url.clone(), "Target");
    let is_candidate = is_file_candidate(
        &index,
        &file_url,
        &target,
        Some("com.example"),
        true,
        source,
    );
    assert!(is_candidate, "Declaring file must always be retained");
}

#[test]
fn test_candidate_pruning_retains_same_package() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Target.kt").unwrap();
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_source = "package com.example\n\nval item: Target = Target()";
    index.index_source(consumer_url.clone(), consumer_source);

    let target = create_test_target(declaring_url, "Target");
    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("com.example"),
        true,
        consumer_source,
    );
    assert!(
        is_candidate,
        "File in the same package must be considered a candidate"
    );
}

#[test]
fn test_candidate_pruning_retains_explicit_import() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Target.kt").unwrap();
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_source =
        "package com.other\n\nimport com.example.Target\n\nval item: Target = Target()";
    index.index_source(consumer_url.clone(), consumer_source);

    let target = create_test_target(declaring_url, "Target");
    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("com.example"),
        true,
        consumer_source,
    );
    assert!(
        is_candidate,
        "File importing the target symbol must be considered a candidate"
    );
}

#[test]
fn test_candidate_pruning_retains_star_import() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Target.kt").unwrap();
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_source =
        "package com.other\n\nimport com.example.*\n\nval item: Target = Target()";
    index.index_source(consumer_url.clone(), consumer_source);

    let target = create_test_target(declaring_url, "Target");
    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("com.example"),
        true,
        consumer_source,
    );
    assert!(
        is_candidate,
        "File star-importing the target package must be considered a candidate"
    );
}

#[test]
fn test_candidate_pruning_retains_implicit_kotlin_package() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Collections.kt").unwrap();
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_source = "package com.other\n\nval items: List<String> = emptyList()";
    index.index_source(consumer_url.clone(), consumer_source);

    let target = create_test_target(declaring_url, "emptyList");
    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("kotlin.collections"),
        true,
        consumer_source,
    );
    assert!(
        is_candidate,
        "Symbols in default kotlin packages must not be pruned across packages"
    );
}

#[test]
fn test_candidate_pruning_rejects_unrelated_package_import() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Target.kt").unwrap();
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_source =
        "package com.other\n\nimport com.unrelated.Target\n\nval item: Target = Target()";
    index.index_source(consumer_url.clone(), consumer_source);

    let target = create_test_target(declaring_url, "Target");
    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("com.example"),
        true,
        consumer_source,
    );
    assert!(
        !is_candidate,
        "File importing Target from unrelated package must be pruned"
    );
}

#[test]
fn test_candidate_pruning_retains_fully_qualified_reference() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Target.kt").unwrap();
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_source =
        "package com.other\n\nval item: com.example.Target = com.example.Target()";
    index.index_source(consumer_url.clone(), consumer_source);

    let target = create_test_target(declaring_url, "Target");
    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("com.example"),
        true,
        consumer_source,
    );
    assert!(
        is_candidate,
        "File containing fully qualified package name must be retained"
    );
}

#[test]
fn test_candidate_pruning_supertype_suffix_requires_dot_boundary() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Node.kt").unwrap();
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_source = "package com.other\n\nval item = AstNode().evaluate()";
    index.index_source(consumer_url.clone(), consumer_source);

    // Register AstNode with an unrelated supertype
    index
        .class_supertypes
        .insert("AstNode".to_string(), vec!["AstNode".to_string()]);

    let mut target = create_test_target(declaring_url, "evaluate");
    target.container_name = Some(smol_str::SmolStr::new("Node"));

    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("com.example"),
        false,
        consumer_source,
    );
    assert!(
        !is_candidate,
        "Supertype AstNode must not match Node container without dot boundary"
    );
}

#[test]
fn test_candidate_pruning_rejects_unimported_extension_function() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Extensions.kt").unwrap();
    let declaring_code = "package com.example.utils\n\nfun String.customTrim(): String = this";
    index.index_source(declaring_url.clone(), declaring_code);

    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_code = "package com.other\n\nval s = \"hello\"\nfun work() { s.customTrim() }";
    index.index_source(consumer_url.clone(), consumer_code);

    let mut target = create_test_target(declaring_url, "customTrim");
    target.container_name = Some(smol_str::SmolStr::new("kotlin.String"));
    target.is_extension = true;

    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("com.example.utils"),
        false,
        consumer_code,
    );
    assert!(
        !is_candidate,
        "Unimported extension function must be pruned even if receiver type appears in source"
    );
}

#[test]
fn test_candidate_pruning_matches_generic_supertype_container() {
    let mut index = WorkspaceIndex::new();
    let declaring_url = Url::parse("file:///project/Node.kt").unwrap();
    let consumer_url = Url::parse("file:///project/Consumer.kt").unwrap();
    let consumer_source = "package com.other\n\nval item = AstNode().evaluate()";
    index.index_source(consumer_url.clone(), consumer_source);

    index.class_supertypes.insert(
        "AstNode".to_string(),
        vec!["com.example.Node<String>".to_string()],
    );

    let mut target = create_test_target(declaring_url, "evaluate");
    target.container_name = Some(smol_str::SmolStr::new("Node"));

    let is_candidate = is_file_candidate(
        &index,
        &consumer_url,
        &target,
        Some("com.example"),
        false,
        consumer_source,
    );
    assert!(
        is_candidate,
        "Class with generic supertype Node<String> must match Node container"
    );
}

use lsp::features::find_references;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_references_primary_constructor_parameter_property_override() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/ConstructorPropertyOverride.kt").unwrap();
    let code = r#"
package com.example

interface ServiceConfig {
    val timeoutMs: Long
}

class DefaultServiceConfig(override val timeoutMs: Long) : ServiceConfig
"#;
    index.index_source(file_url.clone(), code);

    // 1. Cursor at ServiceConfig.timeoutMs declaration (line 4, col 8)
    let base_pos = Position::new(4, 8);
    let refs_from_base = find_references(&index, &file_url, "timeoutMs", base_pos, code, true);

    assert_eq!(
        refs_from_base.len(),
        2,
        "ServiceConfig.timeoutMs references should include interface and constructor parameter override"
    );
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_base.iter().any(|loc| loc.range.start.line == 7));

    // 2. Cursor at constructor parameter override (line 7, col 44)
    let ctor_pos = Position::new(7, 44);
    let refs_from_ctor = find_references(&index, &file_url, "timeoutMs", ctor_pos, code, true);

    assert_eq!(
        refs_from_ctor.len(),
        2,
        "DefaultServiceConfig constructor parameter override should find itself and overridden property"
    );
    assert!(refs_from_ctor.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_ctor.iter().any(|loc| loc.range.start.line == 7));
}

#[test]
fn test_references_override_excludes_regular_constructor_parameter() {
    let mut index = WorkspaceIndex::new();
    let file_url = Url::parse("file:///project/RegularConstructorParam.kt").unwrap();
    let code = r#"
package com.example

interface Counter {
    val count: Int
}

open class BaseCounter(count: Int)

class CustomCounter(override val count: Int) : BaseCounter(count), Counter
"#;
    index.index_source(file_url.clone(), code);

    // Cursor at Counter.count declaration (line 4, col 8)
    let counter_pos = Position::new(4, 8);
    let refs = find_references(&index, &file_url, "count", counter_pos, code, true);

    assert_eq!(
        refs.len(),
        2,
        "Counter.count references must only include Counter.count and CustomCounter.count, NOT BaseCounter(count: Int)"
    );
    assert!(refs.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs.iter().any(|loc| loc.range.start.line == 9));
    assert!(!refs.iter().any(|loc| loc.range.start.line == 7));

    // 2. Cursor at CustomCounter.count override (line 9, col 35)
    let custom_pos = Position::new(9, 35);
    let refs_from_custom = find_references(&index, &file_url, "count", custom_pos, code, true);
    assert_eq!(
        refs_from_custom.len(),
        3,
        "CustomCounter.count references should include interface, override, and BaseCounter(count) super call, NOT BaseCounter(count: Int) declaration"
    );
    assert!(refs_from_custom.iter().any(|loc| loc.range.start.line == 4));
    assert!(refs_from_custom.iter().any(|loc| loc.range.start.line == 9));
    assert!(!refs_from_custom.iter().any(|loc| loc.range.start.line == 7));
}

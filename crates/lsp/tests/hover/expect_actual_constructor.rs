use lsp::features::find_hover;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_actual_primary_constructor_property_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/UserProfile.kt").unwrap();
    let common_source = r#"package com.example

/**
 * User account profile representation.
 */
expect class UserProfile(
    /**
     * Unique identifier of the user account.
     */
    val userId: String,
)
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/UserProfile.kt").unwrap();
    let actual_source = r#"package com.example

actual class UserProfile actual constructor(
    actual val userId: String,
)

fun testUsage(profile: UserProfile) {
    val id = profile.userId
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let decl_pos = Position::new(3, 15);
    let decl_hover = find_hover(&index, &actual_url, "userId", decl_pos, actual_source)
        .expect("hover on actual primary constructor property declaration should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = decl_hover.contents {
        assert!(
            text.contains("Unique identifier of the user account."),
            "Hover on actual primary constructor property declaration should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    let usage_pos = Position::new(7, 21);
    let usage_hover = find_hover(&index, &actual_url, "userId", usage_pos, actual_source)
        .expect("hover on actual primary constructor property usage should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Unique identifier of the user account."),
            "Hover on actual primary constructor property usage should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[test]
fn test_hover_actual_constructor_resolves_expect_doc() {
    let mut index = WorkspaceIndex::new();
    let common_url = Url::parse("file:///project/commonMain/SecureChannel.kt").unwrap();
    let common_source = r#"package com.example

expect class SecureChannel {
    /**
     * Initializes secure channel with an encryption key.
     */
    constructor(key: String)
}
"#;
    index.index_source(common_url.clone(), common_source);

    let actual_url = Url::parse("file:///project/jvmMain/SecureChannel.kt").unwrap();
    let actual_source = r#"package com.example

actual class SecureChannel {
    actual constructor(key: String) {}
}

fun testUsage() {
    val channel = SecureChannel("secret")
}
"#;
    index.index_source(actual_url.clone(), actual_source);

    let usage_pos = Position::new(7, 18);
    let usage_hover = find_hover(
        &index,
        &actual_url,
        "SecureChannel",
        usage_pos,
        actual_source,
    )
    .expect("hover on constructor call site should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = usage_hover.contents {
        assert!(
            text.contains("Initializes secure channel with an encryption key."),
            "Hover on constructor call site should contain expect doc, actual:\n{}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

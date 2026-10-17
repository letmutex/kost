use lsp::features::find_completions;
use lsp::indexer::WorkspaceIndex;
use tower_lsp::lsp_types::{Position, Url};

#[test]
fn test_import_aware_receiver_completion() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/ModifierA.kt").unwrap();
    let file_a_code = r#"package example.base

public interface Modifier2 {
    companion object : Modifier2 {}
}

fun Modifier2.padding(value: Int): Modifer2 {
    return this
}
"#;

    let file_b_url = Url::parse("file:///project/ModifierB.kt").unwrap();
    let file_b_code = r#"package example.other

class Modifier2 {
    companion object {
        const val A_FIELD = 10
    }
}
"#;

    let file_c_url = Url::parse("file:///project/FileC.kt").unwrap();
    let file_c_code = r#"import example.base.Modifier2

Modifier2.

example.other.Modifier2.
"#;

    index.index_source(file_a_url, file_a_code);
    index.index_source(file_b_url, file_b_code);
    index.index_source(file_c_url.clone(), file_c_code);

    // Completion after `Modifier2.` at line 2, col 10
    let pos_imported = Position::new(2, 10);
    let completions_imported = find_completions(&index, &file_c_url, pos_imported, file_c_code);
    let labels_imported: Vec<String> = completions_imported.into_iter().map(|c| c.label).collect();

    assert!(
        labels_imported.contains(&"padding".to_string()),
        "Modifier2. should offer 'padding' from imported example.base.Modifier2: {:?}",
        labels_imported
    );
    assert!(
        !labels_imported.contains(&"A_FIELD".to_string()),
        "Modifier2. should NOT offer 'A_FIELD' from unimported example.other.Modifier2: {:?}",
        labels_imported
    );

    // Completion after `example.other.Modifier2.` at line 4, col 24
    let pos_qualified = Position::new(4, 24);
    let completions_qualified = find_completions(&index, &file_c_url, pos_qualified, file_c_code);
    let labels_qualified: Vec<String> =
        completions_qualified.into_iter().map(|c| c.label).collect();

    assert!(
        labels_qualified.contains(&"A_FIELD".to_string()),
        "example.other.Modifier2. should offer 'A_FIELD': {:?}",
        labels_qualified
    );
    assert!(
        !labels_qualified.contains(&"padding".to_string()),
        "example.other.Modifier2. should NOT offer 'padding': {:?}",
        labels_qualified
    );
}

#[test]
fn test_unrelated_class_supertypes_do_not_leak_to_imported_receiver() {
    let mut index = WorkspaceIndex::new();
    let compose_modifier_url = Url::parse("file:///project/ComposeModifier.kt").unwrap();
    let compose_modifier_code = r#"package androidx.compose.ui

public interface Modifier {
    companion object : Modifier {}
}

fun Modifier.fillMaxSize(): Modifier = this
"#;

    let protobuf_url = Url::parse("file:///project/Protobuf.kt").unwrap();
    let protobuf_code = r#"package androidx.datastore.preferences.protobuf

open class Enum {
    val NAME_FIELD_NUMBER = 1
}

class Modifier : Enum() {
    val OTHER_FIELD = 2
}
"#;

    let usage_url = Url::parse("file:///project/Usage.kt").unwrap();
    let usage_code = r#"import androidx.compose.ui.Modifier

fun main() {
    Modifier.
}
"#;

    index.index_source(compose_modifier_url, compose_modifier_code);
    index.index_source(protobuf_url, protobuf_code);
    index.index_source(usage_url.clone(), usage_code);

    let pos = Position::new(3, 13);
    let completions = find_completions(&index, &usage_url, pos, usage_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"fillMaxSize".to_string()),
        "Modifier. should offer 'fillMaxSize': {:?}",
        labels
    );
    assert!(
        !labels.contains(&"NAME_FIELD_NUMBER".to_string()),
        "Modifier. should NOT offer 'NAME_FIELD_NUMBER' from unrelated protobuf Enum supertype: {:?}",
        labels
    );
    assert!(
        !labels.contains(&"OTHER_FIELD".to_string()),
        "Modifier. should NOT offer 'OTHER_FIELD' from unrelated protobuf Modifier: {:?}",
        labels
    );
}

#[test]
fn test_extension_function_from_other_package_completion() {
    let mut index = WorkspaceIndex::new();
    let compose_modifier_url = Url::parse("file:///project/ComposeModifier.kt").unwrap();
    let compose_modifier_code = r#"package androidx.compose.ui

public interface Modifier {
    companion object : Modifier {}
}
"#;

    let foundation_layout_url = Url::parse("file:///project/FoundationLayout.kt").unwrap();
    let foundation_layout_code = r#"package androidx.compose.foundation.layout

import androidx.compose.ui.Modifier

fun Modifier.padding(value: Int): Modifier = this
fun Modifier.height(value: Int): Modifier = this
"#;

    let usage_url = Url::parse("file:///project/Usage.kt").unwrap();
    let usage_code = r#"import androidx.compose.ui.Modifier
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.height

fun main() {
    Modifier.
}
"#;

    index.index_source(compose_modifier_url, compose_modifier_code);
    index.index_source(foundation_layout_url, foundation_layout_code);
    index.index_source(usage_url.clone(), usage_code);

    let pos = Position::new(5, 13);
    let completions = find_completions(&index, &usage_url, pos, usage_code);
    let labels: Vec<String> = completions.into_iter().map(|c| c.label).collect();

    assert!(
        labels.contains(&"padding".to_string()),
        "Modifier. should offer 'padding' from foundation.layout: {:?}",
        labels
    );
    assert!(
        labels.contains(&"height".to_string()),
        "Modifier. should offer 'height' from foundation.layout: {:?}",
        labels
    );
}

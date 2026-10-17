use std::sync::Arc;

use lsp::features::find_hover;
use lsp::gradle::config::GradleConfig;
use lsp::gradle::pipeline::MultiStageIndexer;
use lsp::indexer::WorkspaceIndex;
use tokio::sync::RwLock;
use tower_lsp::lsp_types::{HoverContents, MarkedString, Position, Url};

#[test]
fn test_hover_cross_file_source_link() {
    let mut index = WorkspaceIndex::new();
    let file_a_url = Url::parse("file:///project/UserService.kt").unwrap();
    let file_a_code = r#"package com.example

class UserService {
    fun findUser(): String = "John"
}
"#;
    index.index_source(file_a_url.clone(), file_a_code);

    let file_b_url = Url::parse("file:///project/Main.kt").unwrap();
    let file_b_code = r#"package com.example

fun main() {
    val service = UserService()
}
"#;
    index.index_source(file_b_url.clone(), file_b_code);

    // Cross-file hover on UserService from Main.kt
    let pos = Position::new(3, 20);
    let hover = find_hover(&index, &file_b_url, "UserService", pos, file_b_code)
        .expect("hover should resolve across files");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover.contents {
        assert!(
            text.contains("class UserService"),
            "Hover should contain class signature: {}",
            text
        );
        assert!(
            text.contains("Defined in [UserService.kt:3](file:///project/UserService.kt#L3)"),
            "Hover must contain cross-file source link: {}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    // Same-file hover on UserService from UserService.kt should not have footer link
    let pos_same = Position::new(2, 8);
    let hover_same = find_hover(&index, &file_a_url, "UserService", pos_same, file_a_code)
        .expect("hover should resolve in same file");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_same.contents {
        assert!(
            !text.contains("Defined in ["),
            "Same-file hover should not include external file link: {}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

#[tokio::test]
async fn test_hover_jar_and_sdk_archive_links() {
    let fixtures_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("gradle")
        .join("fixtures");
    let project_dir = fixtures_dir.join("project");
    let config = GradleConfig::new()
        .with_gradle_user_home(fixtures_dir.join("gradle_cache"))
        .with_android_sdk_root(fixtures_dir.join("android_sdk"));

    let index = Arc::new(RwLock::new(WorkspaceIndex::new()));
    let _ = MultiStageIndexer::spawn(project_dir.clone(), config.clone(), index.clone())
        .await
        .unwrap();
    let mut index = Arc::try_unwrap(index).ok().unwrap().into_inner();

    let test_url = Url::parse("file:///project/App.kt").unwrap();
    let test_code = r#"package com.example.app

import android.app.Activity

fun run(act: Activity) {
    println("test")
}
"#;
    index.index_source(test_url.clone(), test_code);

    // Hover on Android SDK Activity
    let pos_act = Position::new(2, 19);
    let hover_act = find_hover(&index, &test_url, "Activity", pos_act, test_code)
        .expect("Activity hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_act.contents {
        assert!(
            text.contains("Defined in `Activity.class` in [android.jar]("),
            "Activity hover must contain Android SDK archive link: {}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }

    // Hover on stdlib println
    let pos_print = Position::new(4, 6);
    let hover_print = find_hover(&index, &test_url, "println", pos_print, test_code)
        .expect("println hover should resolve");

    if let HoverContents::Scalar(MarkedString::String(text)) = hover_print.contents {
        assert!(
            text.contains("Defined in `Console.kt` in [kotlin-stdlib-2.0.20-sources.jar]("),
            "println hover must contain stdlib sources jar link: {}",
            text
        );
    } else {
        panic!("unexpected hover contents type");
    }
}

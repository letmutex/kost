use common::editorconfig::ResolvedEditorConfig;
use lint::Linter;

#[test]
fn test_import_ordering_respects_star_layout() {
    let linter = Linter::standard();
    let code = "import android.os.Build\nimport assertk.assertThat\nimport java.net.InetAddress\nimport mockwebserver3.MockResponse\n\nfun main() {\n}\n";

    let ini = "[*.kt]\nij_kotlin_imports_layout = *\n";
    let file = common::editorconfig::EditorConfigFile::parse(ini).unwrap();
    let mut config = ResolvedEditorConfig::new();
    common::editorconfig::apply_file_properties(&file, "src/Main.kt", &mut config);

    let report = linter.lint_str_with_config(code, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:import-ordering"),
        "Unexpected import-ordering violation under star import layout: {:?}",
        report.diagnostics()
    );
}

#[test]
fn test_import_ordering_respects_custom_multi_pattern_layout() {
    let linter = Linter::standard();
    let valid_code = "import java.util.List\nimport android.os.Build\nimport com.example.Foo\nimport com.example.Bar as BarAlias\n\nfun main() {}\n";

    let ini = "[*.kt]\nij_kotlin_imports_layout = java.**,android.**,*,^\n";
    let file = common::editorconfig::EditorConfigFile::parse(ini).unwrap();
    let mut config = ResolvedEditorConfig::new();
    common::editorconfig::apply_file_properties(&file, "src/Main.kt", &mut config);

    let report = linter.lint_str_with_config(valid_code, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:import-ordering"),
        "Unexpected import-ordering violation under custom layout: {:?}",
        report.diagnostics()
    );

    let invalid_code = "import android.os.Build\nimport java.util.List\n\nfun main() {}\n";
    let invalid_report =
        linter.lint_str_with_config(invalid_code, Some("src/Main.kt"), Some(&config));
    assert!(
        invalid_report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:import-ordering"),
        "Expected import-ordering violation when java.** follows android.** under layout: {:?}",
        invalid_report.diagnostics()
    );
}

#[test]
fn test_import_ordering_distinguishes_direct_and_recursive_wildcards() {
    let linter = Linter::standard();
    // Layout: android.* (direct only), android.sub.** (recursive), * (all other)
    // android.R belongs to android.* (group 0)
    // android.sub.Nested belongs to android.sub.** (group 1)
    // android.other.Pkg does not match android.* because it has subpackage "other.Pkg", so falls into * (group 2)
    let valid_code =
        "import android.R\nimport android.sub.Nested\nimport android.other.Pkg\n\nfun main() {}\n";

    let ini = "[*.kt]\nij_kotlin_imports_layout = android.*,android.sub.**,*\n";
    let file = common::editorconfig::EditorConfigFile::parse(ini).unwrap();
    let mut config = ResolvedEditorConfig::new();
    common::editorconfig::apply_file_properties(&file, "src/Main.kt", &mut config);

    let report = linter.lint_str_with_config(valid_code, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:import-ordering"),
        "Unexpected import-ordering violation: {:?}",
        report.diagnostics()
    );
}

#[test]
fn test_import_ordering_fallback_without_star_sorts_at_end() {
    let linter = Linter::standard();
    // Layout: java.**, android.** (no * pattern)
    // Unmatched imports (e.g. zed.Foo) should be placed at the end (group 2).
    let valid_code =
        "import java.util.List\nimport android.os.Build\nimport zed.Foo\n\nfun main() {}\n";

    let ini = "[*.kt]\nij_kotlin_imports_layout = java.**,android.**\n";
    let file = common::editorconfig::EditorConfigFile::parse(ini).unwrap();
    let mut config = ResolvedEditorConfig::new();
    common::editorconfig::apply_file_properties(&file, "src/Main.kt", &mut config);

    let report = linter.lint_str_with_config(valid_code, Some("src/Main.kt"), Some(&config));
    assert!(
        !report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:import-ordering"),
        "Unexpected import-ordering violation when unmatched import is at the end: {:?}",
        report.diagnostics()
    );

    let invalid_code =
        "import zed.Foo\nimport java.util.List\nimport android.os.Build\n\nfun main() {}\n";
    let invalid_report =
        linter.lint_str_with_config(invalid_code, Some("src/Main.kt"), Some(&config));
    assert!(
        invalid_report
            .diagnostics()
            .iter()
            .any(|d| d.rule_id == "standard:import-ordering"),
        "Expected import-ordering violation when unmatched import precedes explicitly listed groups: {:?}",
        invalid_report.diagnostics()
    );
}

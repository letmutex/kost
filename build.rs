use std::path::Path;

#[path = "build_lint.rs"]
mod build_lint;
use build_lint::{LintRules, collect_rs_files, lint_file};

static LINT_RULES: LintRules = LintRules {
    max_function_lines: 80,
    max_file_lines_warn: 350,
    max_file_lines_error: 400,
    max_indent_spaces: 36,
};

fn main() {
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=crates");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_lint.rs");
    println!("cargo:rerun-if-changed=build_source_masker.rs");

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let root = Path::new(&manifest_dir);

    let mut files = vec![
        root.join("build.rs"),
        root.join("build_lint.rs"),
        root.join("build_source_masker.rs"),
    ];
    collect_rs_files(&root.join("src"), &mut files);
    collect_rs_files(&root.join("crates"), &mut files);

    let mut violations = Vec::new();
    for file in &files {
        lint_file(&LINT_RULES, file, root, &mut violations);
    }

    for v in &violations {
        println!(
            "cargo:warning=[lint:{}] {}:{} - {} Suggestion: {}",
            v.rule, v.file, v.line, v.message, v.suggestion
        );
    }

    if !violations.is_empty() {
        println!(
            "cargo:warning=[lint] Total custom lint warning(s) & error(s): {}",
            violations.len()
        );
        println!(
            "cargo:warning=[lint] For AI: Don't try to fix errors by compressing lines, cargo fmt will destroy it. Fix the code quality."
        );
        println!("cargo:warning=[lint] For AI: Again, don't compress lines to fix lint errors.");
    }

    if violations.iter().any(|v| v.is_error) {
        std::process::exit(1);
    }
}

#![deny(clippy::wildcard_imports)]
#![deny(clippy::enum_glob_use)]

pub mod assertions;
pub mod config;
pub mod environment;
pub mod runner;
pub mod summary;

pub use config::{FeatureKind, ProjectConfig};
pub use environment::WorkspaceEnvironment;
pub use runner::{RunnerOptions, TestResult, locate_fixtures_root, run_all_projects};
pub use summary::print_test_summary;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_lsp_environment_fixtures() {
        let options = RunnerOptions::default();
        let results = run_all_projects(&options).expect("Failed to execute fixture tests");
        assert!(!results.is_empty(), "No fixture tests were executed");
        let all_passed = print_test_summary(&results);
        assert!(all_passed, "One or more LSP fixture assertions failed");
    }
}

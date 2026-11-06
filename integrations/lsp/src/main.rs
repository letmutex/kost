use std::env;
use std::process::ExitCode;

use lsp_integration_tests::config::FeatureKind;
use lsp_integration_tests::runner::{RunnerOptions, run_all_projects};
use lsp_integration_tests::summary::print_test_summary;

fn parse_arguments() -> RunnerOptions {
    let mut options = RunnerOptions::default();
    let mut arguments = env::args().skip(1);

    while let Some(arg) = arguments.next() {
        match arg.as_str() {
            "--project" => {
                options.project_filter = arguments.next();
            }
            "--feature" => {
                if let Some(feature_name) = arguments.next() {
                    let parsed_feature = FeatureKind::from_directory_name(&feature_name);
                    if parsed_feature.is_none() {
                        eprintln!(
                            "Error: Unrecognized feature '{}'. Valid values: type-inference, hover, references",
                            feature_name
                        );
                        std::process::exit(2);
                    }
                    options.feature_filter = parsed_feature;
                }
            }
            "--filter" => {
                options.name_filter = arguments.next();
            }
            "--help" | "-h" => {
                println!("Usage: cargo run -p lsp-integration-tests -- [OPTIONS]");
                println!();
                println!("Options:");
                println!("  --project <name>     Run fixtures only for the specified project");
                println!(
                    "  --feature <kind>     Filter by feature (type-inference, hover, references)"
                );
                println!("  --filter <str>       Filter test cases by name substring");
                println!("  -h, --help           Print help");
                std::process::exit(0);
            }
            unknown => {
                eprintln!(
                    "Error: Unrecognized option '{}'. Use --help for usage information.",
                    unknown
                );
                std::process::exit(2);
            }
        }
    }

    options
}

fn main() -> ExitCode {
    let options = parse_arguments();

    match run_all_projects(&options) {
        Ok(results) => {
            let success = print_test_summary(&results);
            if success {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(err) => {
            eprintln!("Error executing LSP environment tests: {err}");
            ExitCode::FAILURE
        }
    }
}

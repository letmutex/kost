use common::concat_str;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::assertions::{
    evaluate_hover_test, evaluate_references_test, evaluate_type_inference_test,
};
use crate::config::{
    FeatureKind, HoverFixture, ReferencesFixture, TypeInferenceFixture, load_project_config,
};
use crate::environment::WorkspaceEnvironment;

#[derive(Debug, Clone)]
pub struct TestResult {
    pub project_name: String,
    pub feature: FeatureKind,
    pub fixture_file: String,
    pub test_name: String,
    pub duration: Duration,
    pub outcome: Result<(), String>,
}

#[derive(Debug, Default, Clone)]
pub struct RunnerOptions {
    pub project_filter: Option<String>,
    pub feature_filter: Option<FeatureKind>,
    pub name_filter: Option<String>,
}

pub fn locate_fixtures_root() -> Result<PathBuf, String> {
    if let Ok(dir_from_env) = std::env::var("LSP_FIXTURES_DIR") {
        let path = PathBuf::from(dir_from_env);
        if path.exists() {
            return Ok(path);
        }
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    if manifest_dir.exists() {
        return Ok(manifest_dir);
    }

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidates = [
        current_dir.join("fixtures"),
        current_dir
            .join("integrations")
            .join("lsp")
            .join("fixtures"),
        current_dir.join("lsp").join("fixtures"),
    ];

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(concat_str!(
        "Failed to locate fixtures directory. Searched in CARGO_MANIFEST_DIR/fixtures and current_dir."
    ))
}

pub fn run_all_projects(options: &RunnerOptions) -> Result<Vec<TestResult>, String> {
    let fixtures_root = locate_fixtures_root()?;
    println!("Discovering LSP fixtures in: {}", fixtures_root.display());

    let entries = std::fs::read_dir(&fixtures_root)
        .map_err(|err| concat_str!("Failed to read fixtures root: ", &err.to_string()))?;

    let mut all_results = Vec::new();
    let mut discovered_project_count = 0;

    for entry_result in entries {
        let entry = entry_result.map_err(|err| err.to_string())?;
        let project_dir = entry.path();
        if !project_dir.is_dir() {
            continue;
        }

        let config_file = project_dir.join("project.toml");
        if !config_file.exists() {
            continue;
        }

        let project_config = load_project_config(&config_file)?;
        if let Some(ref filter) = options.project_filter
            && !project_config.name.eq_ignore_ascii_case(filter)
        {
            continue;
        }

        discovered_project_count += 1;
        println!("\nLoading project environment: {}", project_config.name);
        let environment = WorkspaceEnvironment::load_project(&config_file, &project_config)?;

        let project_results = run_project_tests(&environment, &project_dir, options)?;
        all_results.extend(project_results);
    }

    if discovered_project_count == 0 {
        return Err(concat_str!(
            "No matching projects with project.toml found in: ",
            &fixtures_root.to_string_lossy()
        ));
    }

    Ok(all_results)
}

fn run_project_tests(
    environment: &WorkspaceEnvironment,
    project_dir: &Path,
    options: &RunnerOptions,
) -> Result<Vec<TestResult>, String> {
    let mut results = Vec::new();

    let features = [
        FeatureKind::TypeInference,
        FeatureKind::Hover,
        FeatureKind::References,
    ];

    for feature in features {
        if let Some(feature_filter) = options.feature_filter
            && feature_filter != feature
        {
            continue;
        }

        let feature_dir = project_dir.join(feature.directory_name());
        if feature_dir.exists() {
            process_feature_directory(environment, &feature_dir, feature, options, &mut results)?;
        }
    }

    Ok(results)
}

fn process_feature_directory(
    environment: &WorkspaceEnvironment,
    feature_dir: &Path,
    feature: FeatureKind,
    options: &RunnerOptions,
    results: &mut Vec<TestResult>,
) -> Result<(), String> {
    let fixture_entries = std::fs::read_dir(feature_dir)
        .map_err(|err| concat_str!("Failed to read feature directory: ", &err.to_string()))?;

    for entry_result in fixture_entries {
        let entry = entry_result.map_err(|err| err.to_string())?;
        let fixture_path = entry.path();
        if fixture_path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }

        let file_name = fixture_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        let file_content = std::fs::read_to_string(&fixture_path).map_err(|err| {
            concat_str!(
                "Failed to read fixture file ",
                &fixture_path.to_string_lossy(),
                ": ",
                &err.to_string()
            )
        })?;

        match feature {
            FeatureKind::TypeInference => {
                execute_type_inference_fixtures(
                    environment,
                    &file_name,
                    &file_content,
                    options,
                    results,
                )?;
            }
            FeatureKind::Hover => {
                execute_hover_fixtures(environment, &file_name, &file_content, options, results)?;
            }
            FeatureKind::References => {
                execute_references_fixtures(
                    environment,
                    &file_name,
                    &file_content,
                    options,
                    results,
                )?;
            }
        }
    }

    Ok(())
}

fn execute_type_inference_fixtures(
    environment: &WorkspaceEnvironment,
    file_name: &str,
    file_content: &str,
    options: &RunnerOptions,
    results: &mut Vec<TestResult>,
) -> Result<(), String> {
    let fixture: TypeInferenceFixture = toml::from_str(file_content).map_err(|err| {
        concat_str!(
            "Failed to parse type inference fixture in ",
            file_name,
            ": ",
            &err.to_string()
        )
    })?;

    for test_case in fixture.tests {
        if let Some(ref name_filter) = options.name_filter
            && !test_case.name.contains(name_filter)
        {
            continue;
        }

        let start = Instant::now();
        let outcome = evaluate_type_inference_test(environment, &test_case);
        let duration = start.elapsed();

        results.push(TestResult {
            project_name: environment.project_name.clone(),
            feature: FeatureKind::TypeInference,
            fixture_file: file_name.to_string(),
            test_name: test_case.name,
            duration,
            outcome,
        });
    }

    Ok(())
}

fn execute_hover_fixtures(
    environment: &WorkspaceEnvironment,
    file_name: &str,
    file_content: &str,
    options: &RunnerOptions,
    results: &mut Vec<TestResult>,
) -> Result<(), String> {
    let fixture: HoverFixture = toml::from_str(file_content).map_err(|err| {
        concat_str!(
            "Failed to parse hover fixture in ",
            file_name,
            ": ",
            &err.to_string()
        )
    })?;

    for test_case in fixture.tests {
        if let Some(ref name_filter) = options.name_filter
            && !test_case.name.contains(name_filter)
        {
            continue;
        }

        let start = Instant::now();
        let outcome = evaluate_hover_test(environment, &test_case);
        let duration = start.elapsed();

        results.push(TestResult {
            project_name: environment.project_name.clone(),
            feature: FeatureKind::Hover,
            fixture_file: file_name.to_string(),
            test_name: test_case.name,
            duration,
            outcome,
        });
    }

    Ok(())
}

fn execute_references_fixtures(
    environment: &WorkspaceEnvironment,
    file_name: &str,
    file_content: &str,
    options: &RunnerOptions,
    results: &mut Vec<TestResult>,
) -> Result<(), String> {
    let fixture: ReferencesFixture = toml::from_str(file_content).map_err(|err| {
        concat_str!(
            "Failed to parse references fixture in ",
            file_name,
            ": ",
            &err.to_string()
        )
    })?;

    for test_case in fixture.tests {
        if let Some(ref name_filter) = options.name_filter
            && !test_case.name.contains(name_filter)
        {
            continue;
        }

        let start = Instant::now();
        let outcome = evaluate_references_test(environment, &test_case);
        let duration = start.elapsed();

        results.push(TestResult {
            project_name: environment.project_name.clone(),
            feature: FeatureKind::References,
            fixture_file: file_name.to_string(),
            test_name: test_case.name,
            duration,
            outcome,
        });
    }

    Ok(())
}

use std::path::PathBuf;

use serde_json::json;

use lsp::config::{
    IndexingLogLevel, InteractionLogLevel, ServerConfig, VariableHoverTypeInference,
};
use lsp::gradle::archive::ArchiveProcessingMetric;
use lsp::gradle::cache_resolver::ResolvedArtifact;
use lsp::gradle::dependency::DependencyCoordinate;
use lsp::gradle::pipeline::IndexingSummary;

#[test]
fn test_default_server_config() {
    let config = ServerConfig::default();
    assert_eq!(config.indexing_logging, IndexingLogLevel::Default);
    assert_eq!(config.interaction_logging, InteractionLogLevel::None);
    assert_eq!(
        config.variable_hover_type_inference,
        VariableHoverTypeInference::Auto
    );
    assert!(config.indexing_logging.is_enabled());
    assert!(!config.indexing_logging.is_verbose());
    assert!(!config.interaction_logging.is_enabled());
    assert!(!config.variable_hover_type_inference.is_enabled());
    assert!(config.auto_import);
    assert!(!config.work_done_progress_supported);
}

#[test]
fn test_server_config_from_json() {
    let json_val = json!({
        "indexing_logging": "none",
        "interaction_logging": "default"
    });
    let config = ServerConfig::from_init_options(Some(&json_val));
    assert_eq!(config.indexing_logging, IndexingLogLevel::None);
    assert_eq!(config.interaction_logging, InteractionLogLevel::Default);
    assert!(!config.indexing_logging.is_enabled());
    assert!(config.interaction_logging.is_enabled());

    let json_verbose = json!({
        "indexing_logging": "verbose",
        "interaction_logging": "none"
    });
    let config_verbose = ServerConfig::from_init_options(Some(&json_verbose));
    assert_eq!(config_verbose.indexing_logging, IndexingLogLevel::Verbose);
    assert_eq!(
        config_verbose.interaction_logging,
        InteractionLogLevel::None
    );
    assert!(config_verbose.indexing_logging.is_verbose());
    assert!(!config_verbose.interaction_logging.is_enabled());
    let json_infer = json!({
        "variable_hover_type_inference": "always"
    });
    let config_infer = ServerConfig::from_init_options(Some(&json_infer));
    assert_eq!(
        config_infer.variable_hover_type_inference,
        VariableHoverTypeInference::Always
    );
    assert!(config_infer.variable_hover_type_inference.is_enabled());
    assert!(!config_infer.variable_hover_type_inference.is_full_name());

    let json_full = json!({
        "variable_hover_type_inference": "always_full_name"
    });
    let config_full = ServerConfig::from_init_options(Some(&json_full));
    assert_eq!(
        config_full.variable_hover_type_inference,
        VariableHoverTypeInference::AlwaysFullName
    );
    assert!(config_full.variable_hover_type_inference.is_enabled());
    assert!(config_full.variable_hover_type_inference.is_full_name());

    let json_trace = json!({
        "variable_hover_type_inference": "always_with_trace"
    });
    let config_trace = ServerConfig::from_init_options(Some(&json_trace));
    assert_eq!(
        config_trace.variable_hover_type_inference,
        VariableHoverTypeInference::AlwaysWithTrace
    );
    assert!(config_trace.variable_hover_type_inference.is_enabled());
    assert!(!config_trace.variable_hover_type_inference.is_full_name());
    assert!(
        config_trace
            .variable_hover_type_inference
            .is_trace_enabled()
    );

    let json_no_import = json!({
        "auto_import": false
    });
    let config_no_import = ServerConfig::from_init_options(Some(&json_no_import));
    assert!(!config_no_import.auto_import);

    let json_no_import_str = json!({
        "completion_auto_import": "false"
    });
    let config_no_import_str = ServerConfig::from_init_options(Some(&json_no_import_str));
    assert!(!config_no_import_str.auto_import);
}

#[test]
fn test_exact_string_parsing() {
    assert_eq!(
        IndexingLogLevel::parse_exact("none"),
        Some(IndexingLogLevel::None)
    );
    assert_eq!(
        IndexingLogLevel::parse_exact("default"),
        Some(IndexingLogLevel::Default)
    );
    assert_eq!(
        IndexingLogLevel::parse_exact("verbose"),
        Some(IndexingLogLevel::Verbose)
    );

    assert_eq!(
        InteractionLogLevel::parse_exact("none"),
        Some(InteractionLogLevel::None)
    );
    assert_eq!(
        InteractionLogLevel::parse_exact("default"),
        Some(InteractionLogLevel::Default)
    );

    assert_eq!(
        VariableHoverTypeInference::parse_exact("auto"),
        Some(VariableHoverTypeInference::Auto)
    );
    assert_eq!(
        VariableHoverTypeInference::parse_exact("always"),
        Some(VariableHoverTypeInference::Always)
    );
    assert_eq!(
        VariableHoverTypeInference::parse_exact("always_full_name"),
        Some(VariableHoverTypeInference::AlwaysFullName)
    );
    assert_eq!(
        VariableHoverTypeInference::parse_exact("always_with_trace"),
        Some(VariableHoverTypeInference::AlwaysWithTrace)
    );
}

#[test]
fn test_indexing_summary_print_all_modes() {
    let mut summary = IndexingSummary {
        unique_dependencies_indexed: 5,
        transitive_dependencies_resolved: 2,
        descriptors_parsed: 1,
        android_platform: Some("android-35".to_string()),
        android_sdk_jar: Some(PathBuf::from("/mock/android.jar")),
        kotlin_version: Some("2.0.20".to_string()),
        duration: std::time::Duration::from_millis(150),
        ..Default::default()
    };

    summary.resolved_artifacts.push(ResolvedArtifact {
        coordinate: DependencyCoordinate::new("com.example", "lib", Some("1.0.0")),
        binary_path: PathBuf::from("/mock/lib-1.0.0.jar"),
        sources_path: Some(PathBuf::from("/mock/lib-1.0.0-sources.jar")),
    });

    summary.print_resolved_artifacts(IndexingLogLevel::None);
    summary.print_resolved_artifacts(IndexingLogLevel::Default);
    summary.print_resolved_artifacts(IndexingLogLevel::Verbose);
}

#[test]
fn test_indexing_summary_archive_breakdown_and_display_line() {
    let mut summary = IndexingSummary {
        unique_dependencies_indexed: 3,
        graph_dependencies_resolved: 10,
        direct_dependencies_resolved: 1,
        transitive_dependencies_indexed: 2,
        transitive_dependencies_resolved: 9,
        descriptors_parsed: 8,
        android_platform: Some("android-35".to_string()),
        kotlin_version: Some("2.0.20".to_string()),
        total_symbols_indexed: 1500,
        duration: std::time::Duration::from_millis(250),
        ..Default::default()
    };

    summary.archive_metrics.push(ArchiveProcessingMetric {
        path: PathBuf::from("/mock/lib-a.jar"),
        duration: std::time::Duration::from_millis(30),
        symbol_count: 500,
        is_aar: false,
    });
    summary.archive_metrics.push(ArchiveProcessingMetric {
        path: PathBuf::from("/mock/lib-b.aar"),
        duration: std::time::Duration::from_millis(50),
        symbol_count: 600,
        is_aar: true,
    });
    summary.archive_metrics.push(ArchiveProcessingMetric {
        path: PathBuf::from("/mock/android.jar"),
        duration: std::time::Duration::from_millis(40),
        symbol_count: 400,
        is_aar: false,
    });

    assert_eq!(summary.total_archives_indexed(), 3);
    assert_eq!(summary.aar_archives_indexed(), 1);
    assert_eq!(summary.jar_archives_indexed(), 2);

    let display = summary.summary_display_line();
    assert!(
        display.contains("Indexed 3 archives (2 JAR, 1 AAR; 1500 symbols)"),
        "Display line should explicitly state processed archives count and JAR/AAR breakdown: {display}"
    );
    assert!(
        display.contains("across 3 resolved libraries (1 direct, 2 transitive)"),
        "Display line should report resolved libraries breakdown: {display}"
    );
    assert!(
        display.contains("from 10 graph dependencies (parsed 8 descriptors, 7 missing from cache)"),
        "Display line should report graph dependencies and cache misses: {display}"
    );
    assert!(
        display.contains("Android SDK: android-35"),
        "Display line should include Android SDK: {display}"
    );
    assert!(
        display.contains("Kotlin: 2.0.20"),
        "Display line should include Kotlin version: {display}"
    );
}

#[test]
fn test_server_config_serde_default_partial_json() {
    let json_str = r#"{"auto_import": false}"#;
    let config: ServerConfig = serde_json::from_str(json_str).unwrap();
    assert!(!config.auto_import);
    assert_eq!(config.indexing_logging, IndexingLogLevel::Default);
    assert_eq!(config.interaction_logging, InteractionLogLevel::None);
}

#[test]
fn test_server_config_from_json_drops_int_options() {
    let json_int = json!({
        "auto_import": 0
    });
    let config = ServerConfig::from_init_options(Some(&json_int));
    assert!(config.auto_import);

    let json_int_str = json!({
        "auto_import": "0"
    });
    let config_str = ServerConfig::from_init_options(Some(&json_int_str));
    assert!(config_str.auto_import);
}

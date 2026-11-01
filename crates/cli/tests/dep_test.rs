use cli::{CliError, DepArgs, DepSubcommand};

#[test]
fn test_dep_fetch_parsing_with_sources_and_rejected_removed_options() {
    let args = vec![
        "fetch".to_string(),
        "androidx.compose.ui:ui:1.6.0".to_string(),
        "--sources".to_string(),
    ];
    let parsed = DepArgs::parse(&args).unwrap();
    match parsed.subcommand {
        DepSubcommand::Fetch(fetch) => {
            assert_eq!(fetch.coordinates, vec!["androidx.compose.ui:ui:1.6.0"]);
            assert!(fetch.sources);
        }
        _ => panic!("Expected DepSubcommand::Fetch"),
    }

    // Default sources is false when omitted
    let default_fetch = DepArgs::parse(&["fetch".to_string()]).unwrap();
    match default_fetch.subcommand {
        DepSubcommand::Fetch(fetch) => {
            assert!(!fetch.sources);
        }
        _ => panic!("Expected DepSubcommand::Fetch"),
    }

    // Removed options are rejected as unknown
    assert!(matches!(
        DepArgs::parse(&["fetch".to_string(), "--no-sources".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--no-sources"
    ));
    assert!(matches!(
        DepArgs::parse(&["fetch".to_string(), "--no-source".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--no-source"
    ));
    assert!(matches!(
        DepArgs::parse(&["fetch".to_string(), "--concurrency".to_string(), "8".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--concurrency"
    ));
    assert!(matches!(
        DepArgs::parse(&["fetch".to_string(), "-j".to_string(), "8".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "-j"
    ));
}

#[test]
fn test_dep_list_and_tree_maven_repo_and_offline_parsing() {
    let list_args = vec![
        "list".to_string(),
        "--direct".to_string(),
        "--maven-repo".to_string(),
        "https://repo.example.com/maven".to_string(),
        "--offline".to_string(),
        "--gradle-home".to_string(),
        "/tmp/gradle".to_string(),
    ];
    let parsed_list = DepArgs::parse(&list_args).unwrap();
    match parsed_list.subcommand {
        DepSubcommand::List(list) => {
            assert!(list.direct);
            assert!(list.offline);
            assert_eq!(list.gradle_home, Some("/tmp/gradle".to_string()));
            assert_eq!(list.repositories, vec!["https://repo.example.com/maven"]);
        }
        _ => panic!("Expected DepSubcommand::List"),
    }

    let tree_args = vec![
        "tree".to_string(),
        "--depth".to_string(),
        "2".to_string(),
        "--maven-repo".to_string(),
        "https://repo.example.com/maven".to_string(),
        "--offline".to_string(),
    ];
    let parsed_tree = DepArgs::parse(&tree_args).unwrap();
    match parsed_tree.subcommand {
        DepSubcommand::Tree(tree) => {
            assert_eq!(tree.depth, Some(2));
            assert!(tree.offline);
            assert_eq!(tree.repositories, vec!["https://repo.example.com/maven"]);
        }
        _ => panic!("Expected DepSubcommand::Tree"),
    }

    // --json should be rejected as unknown option for fetch, list, and tree
    assert!(matches!(
        DepArgs::parse(&["fetch".to_string(), "--json".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--json"
    ));
    assert!(matches!(
        DepArgs::parse(&["list".to_string(), "--json".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--json"
    ));
    assert!(matches!(
        DepArgs::parse(&["tree".to_string(), "--json".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--json"
    ));

    // Renamed --repo should be rejected as unknown option
    assert!(matches!(
        DepArgs::parse(&["list".to_string(), "--repo".to_string(), "url".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--repo"
    ));
}

#[test]
fn test_dep_root_rejects_bare_options_and_unknown_commands() {
    assert!(matches!(
        DepArgs::parse(&["--offline".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--offline"
    ));
    assert!(matches!(
        DepArgs::parse(&["--json".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--json"
    ));
    assert!(matches!(
        DepArgs::parse(&["-p".to_string(), "dir".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "-p"
    ));
    assert!(matches!(
        DepArgs::parse(&["--maven-repo".to_string(), "https://example.com".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--maven-repo"
    ));
    assert!(matches!(
        DepArgs::parse(&["--gradle-home".to_string(), "/home/gradle".to_string()]),
        Err(CliError::UnknownOption(opt)) if opt == "--gradle-home"
    ));
    assert!(matches!(
        DepArgs::parse(&["androidx.compose.ui:ui:1.6.0".to_string()]),
        Err(CliError::UnknownCommand(cmd)) if cmd == "androidx.compose.ui:ui:1.6.0"
    ));
    assert!(matches!(
        DepArgs::parse(&["unknown-subcommand".to_string()]),
        Err(CliError::UnknownCommand(cmd)) if cmd == "unknown-subcommand"
    ));
}

#[test]
fn test_dep_tree_invalid_depth_returns_error() {
    let invalid_depth = vec![
        "tree".to_string(),
        "--depth".to_string(),
        "not-a-number".to_string(),
    ];
    assert_eq!(
        DepArgs::parse(&invalid_depth),
        Err(CliError::InvalidOptionValue {
            option: "--depth".to_string(),
            value: "not-a-number".to_string(),
        })
    );
}

#[test]
fn test_dep_double_dash_treats_remaining_args_as_coordinates() {
    let args = vec![
        "fetch".to_string(),
        "--sources".to_string(),
        "--".to_string(),
        "org.example:artifact:1.0.0".to_string(),
    ];
    let parsed = DepArgs::parse(&args).unwrap();
    match parsed.subcommand {
        DepSubcommand::Fetch(fetch) => {
            assert!(fetch.sources);
            assert_eq!(fetch.coordinates, vec!["org.example:artifact:1.0.0"]);
        }
        _ => panic!("Expected DepSubcommand::Fetch"),
    }
}

//! CLI argument parsing and options for `kost dep`.

use crate::error::CliError;
use crate::parse_value_option;

mod help;
pub use help::{
    render_dep_help, render_dep_help_styled, render_fetch_help, render_fetch_help_styled,
    render_list_help, render_list_help_styled, render_tree_help, render_tree_help_styled,
};

/// Subcommand actions supported by `kost dep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DepSubcommand {
    Fetch(DepFetchArgs),
    List(DepListArgs),
    Tree(DepTreeArgs),
    Help,
}

/// Arguments for `kost dep fetch` (or `kost dep download`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepFetchArgs {
    pub coordinates: Vec<String>,
    pub project_dir: Option<String>,
    pub sources: bool,
    pub transitive: bool,
    pub repositories: Vec<String>,
    pub gradle_home: Option<String>,
    pub help: bool,
}

impl Default for DepFetchArgs {
    fn default() -> Self {
        Self {
            coordinates: Vec::new(),
            project_dir: None,
            sources: false,
            transitive: true,
            repositories: Vec::new(),
            gradle_home: None,
            help: false,
        }
    }
}

/// Arguments for `kost dep list`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DepListArgs {
    pub coordinates: Vec<String>,
    pub project_dir: Option<String>,
    pub repositories: Vec<String>,
    pub gradle_home: Option<String>,
    pub direct: bool,
    pub cached_only: bool,
    pub missing_only: bool,
    pub offline: bool,
    pub help: bool,
}

/// Arguments for `kost dep tree`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DepTreeArgs {
    pub coordinates: Vec<String>,
    pub project_dir: Option<String>,
    pub repositories: Vec<String>,
    pub gradle_home: Option<String>,
    pub depth: Option<usize>,
    pub filter: Option<String>,
    pub offline: bool,
    pub help: bool,
}

/// Unified argument structure parsed from `kost dep ...`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepArgs {
    pub subcommand: DepSubcommand,
}

impl DepArgs {
    pub fn parse(arguments: &[String]) -> Result<Self, CliError> {
        if arguments.is_empty() {
            return Ok(Self {
                subcommand: DepSubcommand::Help,
            });
        }

        let first = arguments[0].as_str();
        match first {
            "-h" | "--help" => Ok(Self {
                subcommand: DepSubcommand::Help,
            }),
            "help" => {
                if arguments.len() > 1 {
                    match arguments[1].as_str() {
                        "-h" | "--help" | "help" => Ok(Self {
                            subcommand: DepSubcommand::Help,
                        }),
                        "fetch" | "download" | "get" => Ok(Self {
                            subcommand: DepSubcommand::Fetch(DepFetchArgs {
                                help: true,
                                ..Default::default()
                            }),
                        }),
                        "list" | "ls" => Ok(Self {
                            subcommand: DepSubcommand::List(DepListArgs {
                                help: true,
                                ..Default::default()
                            }),
                        }),
                        "tree" | "graph" => Ok(Self {
                            subcommand: DepSubcommand::Tree(DepTreeArgs {
                                help: true,
                                ..Default::default()
                            }),
                        }),
                        other if other.starts_with('-') => {
                            Err(CliError::UnknownOption(other.to_string()))
                        }
                        other => Err(CliError::UnknownCommand(other.to_string())),
                    }
                } else {
                    Ok(Self {
                        subcommand: DepSubcommand::Help,
                    })
                }
            }
            "fetch" | "download" | "get" => {
                let fetch_args = parse_fetch_args(&arguments[1..])?;
                Ok(Self {
                    subcommand: DepSubcommand::Fetch(fetch_args),
                })
            }
            "list" | "ls" => {
                let list_args = parse_list_args(&arguments[1..])?;
                Ok(Self {
                    subcommand: DepSubcommand::List(list_args),
                })
            }
            "tree" | "graph" => {
                let tree_args = parse_tree_args(&arguments[1..])?;
                Ok(Self {
                    subcommand: DepSubcommand::Tree(tree_args),
                })
            }
            unrecognized if unrecognized.starts_with('-') => {
                Err(CliError::UnknownOption(unrecognized.to_string()))
            }
            unrecognized => Err(CliError::UnknownCommand(unrecognized.to_string())),
        }
    }
}

fn parse_common_opt<'a>(
    argument: &str,
    argument_iterator: &mut std::slice::Iter<'a, String>,
    project_dir: &mut Option<String>,
    gradle_home: &mut Option<String>,
    repositories: &mut Vec<String>,
) -> Result<bool, CliError> {
    if let Some(value) = parse_value_option(argument, "--project-dir", argument_iterator)? {
        *project_dir = Some(value);
        Ok(true)
    } else if let Some(value) = parse_value_option(argument, "-p", argument_iterator)? {
        *project_dir = Some(value);
        Ok(true)
    } else if let Some(value) = parse_value_option(argument, "--maven-repo", argument_iterator)? {
        repositories.push(value);
        Ok(true)
    } else if let Some(value) = parse_value_option(argument, "--gradle-home", argument_iterator)? {
        *gradle_home = Some(value);
        Ok(true)
    } else {
        Ok(false)
    }
}

fn parse_fetch_args(arguments: &[String]) -> Result<DepFetchArgs, CliError> {
    let mut args = DepFetchArgs::default();
    let mut argument_iterator = arguments.iter();

    while let Some(argument) = argument_iterator.next() {
        if argument == "--" {
            args.coordinates.extend(argument_iterator.cloned());
            break;
        } else if argument == "-h" || argument == "--help" {
            args.help = true;
        } else if argument == "--sources" {
            args.sources = true;
        } else if argument == "--no-transitive" {
            args.transitive = false;
        } else if parse_common_opt(
            argument,
            &mut argument_iterator,
            &mut args.project_dir,
            &mut args.gradle_home,
            &mut args.repositories,
        )? {
            // Handled common option
        } else if argument.starts_with('-') {
            return Err(CliError::UnknownOption(argument.to_string()));
        } else {
            args.coordinates.push(argument.to_string());
        }
    }

    Ok(args)
}

fn parse_list_args(arguments: &[String]) -> Result<DepListArgs, CliError> {
    let mut args = DepListArgs::default();
    let mut argument_iterator = arguments.iter();

    while let Some(argument) = argument_iterator.next() {
        if argument == "--" {
            args.coordinates.extend(argument_iterator.cloned());
            break;
        } else if argument == "-h" || argument == "--help" {
            args.help = true;
        } else if argument == "--direct" {
            args.direct = true;
        } else if argument == "--cached-only" {
            args.cached_only = true;
        } else if argument == "--missing-only" {
            args.missing_only = true;
        } else if argument == "--offline" {
            args.offline = true;
        } else if parse_common_opt(
            argument,
            &mut argument_iterator,
            &mut args.project_dir,
            &mut args.gradle_home,
            &mut args.repositories,
        )? {
            // Handled common option
        } else if argument.starts_with('-') {
            return Err(CliError::UnknownOption(argument.to_string()));
        } else {
            args.coordinates.push(argument.to_string());
        }
    }

    Ok(args)
}

fn parse_tree_args(arguments: &[String]) -> Result<DepTreeArgs, CliError> {
    let mut args = DepTreeArgs::default();
    let mut argument_iterator = arguments.iter();

    while let Some(argument) = argument_iterator.next() {
        if argument == "--" {
            args.coordinates.extend(argument_iterator.cloned());
            break;
        } else if argument == "-h" || argument == "--help" {
            args.help = true;
        } else if argument == "--offline" {
            args.offline = true;
        } else if let Some(value) = parse_value_option(argument, "--depth", &mut argument_iterator)?
        {
            let parsed_depth =
                value
                    .parse::<usize>()
                    .map_err(|_| CliError::InvalidOptionValue {
                        option: "--depth".to_string(),
                        value,
                    })?;
            args.depth = Some(parsed_depth);
        } else if let Some(value) =
            parse_value_option(argument, "--filter", &mut argument_iterator)?
        {
            args.filter = Some(value);
        } else if parse_common_opt(
            argument,
            &mut argument_iterator,
            &mut args.project_dir,
            &mut args.gradle_home,
            &mut args.repositories,
        )? {
            // Handled common option
        } else if argument.starts_with('-') {
            return Err(CliError::UnknownOption(argument.to_string()));
        } else {
            args.coordinates.push(argument.to_string());
        }
    }

    Ok(args)
}

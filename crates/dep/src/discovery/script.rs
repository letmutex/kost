use ahash::AHashMap;

use super::ast::{
    ParseError, RepositoriesResult, ScriptDialect,
    groovy::{parse_groovy_script, parse_groovy_script_with_repos},
    kotlin::{parse_kotlin_script, parse_kotlin_script_with_repos},
};
use crate::coordinate::DependencyCoordinate;

/// Parse direct declared dependencies from a build script using AST traversal.
pub(crate) fn parse_script_dependencies(
    content: &str,
    properties: &AHashMap<String, String>,
    deps: &mut Vec<DependencyCoordinate>,
    dialect: ScriptDialect,
) -> Result<(), ParseError> {
    match dialect {
        ScriptDialect::Kotlin => parse_kotlin_script(content, properties, deps),
        ScriptDialect::Groovy => parse_groovy_script(content, properties, deps),
    }
}

/// Parse dependencies and repositories in a single AST pass over a build script.
pub(crate) fn parse_script_dependencies_and_repos(
    content: &str,
    properties: &AHashMap<String, String>,
    deps: Option<&mut Vec<DependencyCoordinate>>,
    repos: &mut RepositoriesResult,
    dialect: ScriptDialect,
) -> Result<(), ParseError> {
    match dialect {
        ScriptDialect::Kotlin => {
            parse_kotlin_script_with_repos(content, properties, deps, Some(repos))
        }
        ScriptDialect::Groovy => {
            parse_groovy_script_with_repos(content, properties, deps, Some(repos))
        }
    }
}

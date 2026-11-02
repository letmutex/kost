#![deny(clippy::wildcard_imports)]
#![deny(clippy::too_many_arguments)]

pub mod ast;
pub mod cache;
pub mod cache_resolver;
pub mod catalog;
pub mod coordinate;
pub mod descriptor;
pub mod discovery;
pub mod graph;
pub mod module;
pub mod pom;
pub mod properties;
pub mod repo;
pub mod version;

pub use ast::{BuildScriptInfo, DeclaredRepository, ParseError, RepositoriesResult, ScriptDialect};
pub use cache::GradleCache;
pub use cache_resolver::{
    CacheResolverContext, GradleCacheResolver, ResolvedArtifact, normalize_archive_path,
};
pub use catalog::{VersionCatalog, normalize_catalog_accessor};
pub use coordinate::{DependencyCoordinate, KmpTarget, deduplicate_newest};
pub use descriptor::ModuleDescriptor;
pub use graph::{DependencyGraph, TransitiveGraphResult, resolve_offline_transitives};
pub use module::GradleModuleMetadata;
pub use pom::{PomDependency, PomModel, PomParent};
pub use properties::{collect_gradle_properties, parse_gradle_properties};
pub use repo::{Repository, RepositoryRegistry};
pub use version::{ArtifactVersion, QualifierKind, VersionComponent};

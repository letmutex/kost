pub use resolver::{cache, coordinate, discovery, module, pom, repo, version};

pub mod downloader;
pub mod graph;
pub mod metadata;
pub mod progress;
pub mod renderer;
pub mod runner;

pub use cache::GradleCache;
pub use coordinate::{DependencyCoordinate, KmpTarget, deduplicate_newest};
pub use downloader::Downloader;
pub use graph::{GraphNode, SolvedGraph};
pub use progress::ResolveProgress;
pub use renderer::TreeRenderer;
pub use repo::{Repository, RepositoryRegistry};
pub use resolver::{ParseError, ScriptDialect};
pub use runner::run;
pub use version::ArtifactVersion;

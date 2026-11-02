use ahash::AHashMap;
use std::sync::RwLock;

use crate::coordinate::DependencyCoordinate;
use crate::pom::PomModel;

#[derive(Debug, Default)]
pub(super) struct MemoryCacheState {
    pub(super) resolved_deps: RwLock<AHashMap<DependencyCoordinate, Vec<DependencyCoordinate>>>,
    pub(super) parent_poms: RwLock<AHashMap<DependencyCoordinate, PomModel>>,
}

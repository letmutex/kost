use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::coordinate::DependencyCoordinate;
use crate::pom::PomModel;
use crate::version::ArtifactVersion;

mod fs_util;
mod memory;

pub(crate) use fs_util::is_safe_component;
pub use fs_util::{dirs_home, hex_digest};
use memory::MemoryCacheState;

/// Manages interaction with the canonical local Gradle cache (`~/.gradle/caches/modules-2/files-2.1`).
#[derive(Debug, Clone)]
pub struct GradleCache {
    home_dir: PathBuf,
    files_dir: PathBuf,
    writable: bool,
    memory: Arc<MemoryCacheState>,
}

impl GradleCache {
    pub fn resolve_home_dir(custom_gradle_home: Option<&Path>) -> Option<PathBuf> {
        if let Some(custom_path) = custom_gradle_home {
            Some(custom_path.to_path_buf())
        } else if let Ok(env_home) = std::env::var("GRADLE_USER_HOME") {
            Some(PathBuf::from(env_home))
        } else {
            let home_dir = dirs_home()?;
            Some(home_dir.join(".gradle"))
        }
    }

    pub fn resolve_existing_home_dir(custom_gradle_home: Option<&Path>) -> Option<PathBuf> {
        if let Some(custom_path) = custom_gradle_home
            && custom_path.exists()
        {
            return Some(custom_path.to_path_buf());
        }
        if let Ok(env_home) = std::env::var("GRADLE_USER_HOME") {
            let path = PathBuf::from(env_home);
            if path.exists() {
                return Some(path);
            }
        }
        dirs_home()
            .map(|home| home.join(".gradle"))
            .filter(|path| path.exists())
    }

    pub fn resolve_m2_repository(custom_m2: Option<&Path>) -> Option<PathBuf> {
        if let Some(custom_path) = custom_m2
            && custom_path.exists()
        {
            return Some(custom_path.to_path_buf());
        }
        dirs_home()
            .map(|home| home.join(".m2").join("repository"))
            .filter(|path| path.exists())
    }

    pub fn new(custom_gradle_home: Option<&Path>) -> Option<Self> {
        let home = Self::resolve_home_dir(custom_gradle_home)?;
        let files_dir = home.join("caches").join("modules-2").join("files-2.1");
        Some(Self {
            home_dir: home,
            files_dir,
            writable: true,
            memory: Arc::default(),
        })
    }

    pub fn read_only(mut self) -> Self {
        self.writable = false;
        self
    }

    pub fn with_writable(mut self, writable: bool) -> Self {
        self.writable = writable;
        self
    }

    pub fn is_writable(&self) -> bool {
        self.writable
    }

    pub fn get_resolved(&self, coord: &DependencyCoordinate) -> Option<Vec<DependencyCoordinate>> {
        self.memory.resolved_deps.read().ok()?.get(coord).cloned()
    }

    pub fn insert_resolved(&self, coord: DependencyCoordinate, deps: Vec<DependencyCoordinate>) {
        if let Ok(mut lock) = self.memory.resolved_deps.write() {
            lock.insert(coord, deps);
        }
    }

    pub fn get_parent_pom(&self, coord: &DependencyCoordinate) -> Option<PomModel> {
        self.memory.parent_poms.read().ok()?.get(coord).cloned()
    }

    pub fn insert_parent_pom(&self, coord: DependencyCoordinate, pom: PomModel) {
        if let Ok(mut lock) = self.memory.parent_poms.write() {
            lock.insert(coord, pom);
        }
    }

    pub fn get_or_parse_pom(&self, coord: &DependencyCoordinate) -> Option<PomModel> {
        if let Some(pom) = self.get_parent_pom(coord) {
            return Some(pom);
        }
        if let Some(path) = self.find_pom(coord) {
            let pom = fs::read_to_string(path)
                .ok()
                .and_then(|xml| PomModel::parse(&xml).ok())?;
            self.insert_parent_pom(coord.clone(), pom.clone());
            return Some(pom);
        }
        None
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    pub fn files_dir(&self) -> &Path {
        &self.files_dir
    }

    /// Locate an existing artifact (JAR, AAR, or sources) in the Gradle cache.
    pub fn find_artifact(&self, coord: &DependencyCoordinate, is_sources: bool) -> Option<PathBuf> {
        for cand_name in coord.kmp_probe_candidates() {
            let mut cand_coord = coord.clone();
            cand_coord.name = smol_str::SmolStr::new(&cand_name);
            let file_name = cand_coord.artifact_file_name(is_sources);
            if let Some(path) = self.find_file(&cand_coord, &file_name) {
                return Some(path);
            }
            if !is_sources && cand_coord.extension == "jar" {
                let mut aar_coord = cand_coord.clone();
                aar_coord.extension = smol_str::SmolStr::new("aar");
                let aar_name = aar_coord.artifact_file_name(false);
                if let Some(path) = self.find_file(&aar_coord, &aar_name) {
                    return Some(path);
                }
            }
        }
        None
    }

    /// Locate an existing POM file in the Gradle cache.
    pub fn find_pom(&self, coord: &DependencyCoordinate) -> Option<PathBuf> {
        for cand_name in coord.kmp_probe_candidates() {
            let mut cand_coord = coord.clone();
            cand_coord.name = smol_str::SmolStr::new(&cand_name);
            let file_name = cand_coord.pom_file_name();
            if let Some(path) = self.find_file(&cand_coord, &file_name) {
                return Some(path);
            }
        }
        None
    }

    /// Locate an existing `.module` metadata file in the Gradle cache.
    pub fn find_module(&self, coord: &DependencyCoordinate) -> Option<PathBuf> {
        for cand_name in coord.kmp_probe_candidates() {
            let mut cand_coord = coord.clone();
            cand_coord.name = smol_str::SmolStr::new(&cand_name);
            let file_name = cand_coord.module_file_name();
            if let Some(path) = self.find_file(&cand_coord, &file_name) {
                return Some(path);
            }
        }
        None
    }

    fn find_file(&self, coord: &DependencyCoordinate, file_name: &str) -> Option<PathBuf> {
        let ver_str = coord
            .version
            .as_ref()
            .map(|v| v.raw())
            .filter(|s| !s.is_empty())?;

        if !is_safe_component(coord.group.as_str())
            || !is_safe_component(coord.name.as_str())
            || !is_safe_component(ver_str)
            || !is_safe_component(file_name)
        {
            return None;
        }

        let ver_dir = self
            .files_dir
            .join(coord.group.as_str())
            .join(coord.name.as_str())
            .join(ver_str);

        if !ver_dir.is_dir() {
            return None;
        }

        let hash_entries = fs::read_dir(&ver_dir).ok()?;
        for hash_entry in hash_entries.flatten() {
            if hash_entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                let candidate = hash_entry.path().join(file_name);
                if candidate.is_file()
                    && fs::metadata(&candidate)
                        .map(|m| m.len() > 0)
                        .unwrap_or(false)
                {
                    return Some(candidate);
                }
            }
        }

        None
    }

    /// Locate available versions in the local Gradle cache (`files-2.1`) for a given group and artifact name.
    pub fn find_cached_versions(&self, group: &str, name: &str) -> Vec<ArtifactVersion> {
        let mut versions = Vec::new();
        if !is_safe_component(group) || !is_safe_component(name) {
            return versions;
        }
        let artifact_dir = self.files_dir.join(group).join(name);
        if let Ok(entries) = fs::read_dir(artifact_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                    && let Some(ver_str) = entry.file_name().to_str()
                    && is_safe_component(ver_str)
                {
                    versions.push(ArtifactVersion::parse(ver_str));
                }
            }
        }
        versions
    }

    /// Locate available versions in the local Maven repository (`~/.m2/repository`) for a given group and artifact name.
    pub fn find_m2_cached_versions(&self, group: &str, name: &str) -> Vec<ArtifactVersion> {
        let mut versions = Vec::new();
        if !group.split('.').all(is_safe_component) || !is_safe_component(name) {
            return versions;
        }
        if let Some(m2_dir) = Self::resolve_m2_repository(None) {
            let group_path = group.replace('.', "/");
            let artifact_dir = m2_dir.join(group_path).join(name);
            if let Ok(entries) = fs::read_dir(artifact_dir) {
                for entry in entries.flatten() {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                        && let Some(ver_str) = entry.file_name().to_str()
                        && is_safe_component(ver_str)
                    {
                        versions.push(ArtifactVersion::parse(ver_str));
                    }
                }
            }
        }
        versions
    }

    /// Store a downloaded file into the Gradle cache using its SHA-1 hash directory.
    pub fn store_file(
        &self,
        coord: &DependencyCoordinate,
        file_name: &str,
        data: &[u8],
    ) -> Result<PathBuf, String> {
        let mut hasher = sha1_smol::Sha1::new();
        hasher.update(data);
        let sha1_hex = hex_digest(&hasher.digest().bytes());

        let ver_str = coord
            .version
            .as_ref()
            .map(|v| v.raw())
            .filter(|s| !s.is_empty())
            .unwrap_or("unspecified");

        if !is_safe_component(coord.group.as_str())
            || !is_safe_component(coord.name.as_str())
            || !is_safe_component(ver_str)
            || !is_safe_component(file_name)
        {
            return Err(common::concat_str!(
                "Invalid path component in dependency coordinate or file name: '",
                file_name,
                "'"
            ));
        }

        let target_dir = self
            .files_dir
            .join(coord.group.as_str())
            .join(coord.name.as_str())
            .join(ver_str)
            .join(&sha1_hex);
        let final_path = target_dir.join(file_name);

        if !self.writable {
            return Err(common::concat_str!(
                "Cannot store artifact '",
                file_name,
                "': cache is read-only"
            ));
        }

        if final_path.is_file()
            && fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0) == data.len() as u64
        {
            return Ok(final_path);
        }

        fs::create_dir_all(&target_dir).map_err(|err| {
            common::concat_str!(
                "Failed to create cache directory '",
                &target_dir.to_string_lossy(),
                "': ",
                &err.to_string()
            )
        })?;

        fs_util::write_cache_atomic(&target_dir, &final_path, file_name, data)?;
        Ok(final_path)
    }
}

use sha1::{Digest, Sha1};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

fn is_safe_component(component: &str) -> bool {
    !component.is_empty()
        && component != "."
        && !component.contains('/')
        && !component.contains('\\')
        && !component.contains(':')
        && !component.contains('*')
        && !component.contains('?')
        && !component.contains('"')
        && !component.contains('<')
        && !component.contains('>')
        && !component.contains('|')
        && !component.contains("..")
        && !component.contains('\0')
}

use crate::coordinate::DependencyCoordinate;

/// Manages interaction with the canonical local Gradle cache (~/.gradle/caches/modules-2/files-2.1).
#[derive(Debug, Clone)]
pub struct GradleCache {
    home_dir: PathBuf,
    files_dir: PathBuf,
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

    pub fn new(custom_gradle_home: Option<&Path>) -> Option<Self> {
        let home = Self::resolve_home_dir(custom_gradle_home)?;
        let files_dir = home.join("caches").join("modules-2").join("files-2.1");
        Some(Self {
            home_dir: home,
            files_dir,
        })
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    pub fn files_dir(&self) -> &Path {
        &self.files_dir
    }

    /// Locate an existing artifact (JAR or AAR or sources) in the Gradle cache.
    pub fn find_artifact(&self, coord: &DependencyCoordinate, is_sources: bool) -> Option<PathBuf> {
        for cand_name in coord.kmp_candidate_names() {
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
        for cand_name in coord.kmp_candidate_names() {
            let mut cand_coord = coord.clone();
            cand_coord.name = smol_str::SmolStr::new(&cand_name);
            let file_name = cand_coord.pom_file_name();
            if let Some(path) = self.find_file(&cand_coord, &file_name) {
                return Some(path);
            }
        }
        None
    }

    /// Locate an existing .module metadata file in the Gradle cache.
    pub fn find_module(&self, coord: &DependencyCoordinate) -> Option<PathBuf> {
        for cand_name in coord.kmp_candidate_names() {
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

    /// Store a downloaded file into the Gradle cache using its SHA-1 hash directory.
    pub fn store_file(
        &self,
        coord: &DependencyCoordinate,
        file_name: &str,
        data: &[u8],
    ) -> Result<PathBuf, String> {
        let mut hasher = Sha1::new();
        hasher.update(data);
        let sha1_hex = hex_digest(&hasher.finalize());

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

        write_cache_atomic(&target_dir, &final_path, file_name, data)?;
        Ok(final_path)
    }
}

fn write_cache_atomic(
    target_dir: &Path,
    final_path: &Path,
    file_name: &str,
    data: &[u8],
) -> Result<(), String> {
    if final_path.is_file() {
        let _ = fs::remove_file(final_path);
    }

    let seq = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp_name = common::concat_str!(
        ".tmp_",
        &std::process::id().to_string(),
        "_",
        &seq.to_string(),
        "_",
        file_name
    );
    let temp_path = target_dir.join(&temp_name);

    fs::write(&temp_path, data)
        .map_err(|err| common::concat_str!("Failed to write temporary file: ", &err.to_string()))?;

    if let Err(err) = fs::rename(&temp_path, final_path) {
        let _ = fs::remove_file(&temp_path);
        let valid = final_path.is_file()
            && fs::metadata(final_path).map(|m| m.len()).unwrap_or(0) == data.len() as u64;
        if !valid {
            return Err(common::concat_str!(
                "Failed to finalize cache artifact: ",
                &err.to_string()
            ));
        }
    }

    Ok(())
}

pub fn hex_digest(bytes: &[u8]) -> String {
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX_CHARS[(byte >> 4) as usize] as char);
        output.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(crate) fn dirs_home() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(user_profile));
        }
        if let Ok(home_path) = std::env::var("HOME") {
            return Some(PathBuf::from(home_path));
        }
    } else {
        if let Ok(home_path) = std::env::var("HOME") {
            return Some(PathBuf::from(home_path));
        }
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            return Some(PathBuf::from(user_profile));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gradle_cache_store_and_lookup() {
        let temp_dir = std::env::temp_dir().join("kost_test_gradle_cache");
        let _ = fs::remove_dir_all(&temp_dir);

        let cache = GradleCache {
            home_dir: temp_dir.clone(),
            files_dir: temp_dir.join("caches").join("modules-2").join("files-2.1"),
        };

        let coord = DependencyCoordinate::parse("org.example:sample:1.0.0").unwrap();
        let payload = b"sample jar payload content";

        let stored = cache
            .store_file(&coord, "sample-1.0.0.jar", payload)
            .unwrap();
        assert!(stored.is_file());
        assert_eq!(fs::read(&stored).unwrap(), payload);

        let found = cache.find_artifact(&coord, false).unwrap();
        assert_eq!(found, stored);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

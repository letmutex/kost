//! File discovery and path resolution utilities for Kotlin source files.

use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;
use std::path::{Path, PathBuf};

/// Returns true if the path points to a Kotlin source file (`.kt` or `.kts`).
pub fn is_kotlin_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("kt") || ext.eq_ignore_ascii_case("kts"))
}

/// Resolves a set of target Kotlin files from user-supplied paths, globs, or current directory.
///
/// - If `inputs` is empty: searches the current directory recursively, respecting `.gitignore`.
/// - If `inputs` contains direct paths: collects files or walks directories, respecting `.gitignore`.
/// - If `inputs` contains glob patterns (`*`, `?`, `[`): evaluates them via `.gitignore`-aware walk.
pub fn resolve_target_files(inputs: &[String]) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut resolved_files = Vec::new();

    if inputs.is_empty() {
        collect_from_dir(Path::new("."), &mut resolved_files)?;
    } else {
        let mut glob_patterns = Vec::new();

        for input in inputs {
            let path = PathBuf::from(input);
            // Check direct path existence first to avoid misclassifying real files/dirs as globs.
            if path.exists() {
                if path.is_file() {
                    if is_kotlin_file(&path) {
                        resolved_files.push(clean_path(path));
                    }
                } else if path.is_dir() {
                    collect_from_dir(&path, &mut resolved_files)?;
                }
            } else if is_glob_pattern(input) {
                glob_patterns.push(input.as_str());
            } else {
                let mut error_message = String::from("Target path not found: ");
                error_message.push_str(input);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    error_message,
                ));
            }
        }

        if !glob_patterns.is_empty() {
            collect_from_globs(&glob_patterns, &mut resolved_files)?;
        }
    }

    resolved_files.sort();
    resolved_files.dedup();
    Ok(resolved_files)
}

fn is_glob_pattern(input: &str) -> bool {
    // Strip Windows extended-length path prefix (`\\?\`) so verbatim/canonical paths
    // aren't falsely identified as glob patterns due to the embedded '?' character.
    let pattern = input.strip_prefix(r"\\?\").unwrap_or(input);
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

fn clean_path(path: PathBuf) -> PathBuf {
    match path.strip_prefix(".") {
        Ok(stripped) => stripped.to_path_buf(),
        Err(_) => path,
    }
}

fn collect_from_dir(dir: &Path, target: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    let walker = WalkBuilder::new(dir)
        .standard_filters(true)
        .build_parallel();
    run_parallel_walk(walker, target)
}

fn collect_from_globs(patterns: &[&str], target: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    let mut override_builder = OverrideBuilder::new(".");
    for pattern in patterns {
        let clean_pattern = pattern.strip_prefix("./").unwrap_or(pattern);
        override_builder.add(clean_pattern).map_err(|err| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, err.to_string())
        })?;
    }
    let overrides = override_builder
        .build()
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidInput, err.to_string()))?;

    let walker = WalkBuilder::new(".")
        .standard_filters(true)
        .overrides(overrides)
        .build_parallel();

    run_parallel_walk(walker, target)
}

fn run_parallel_walk(
    walker: ignore::WalkParallel,
    target: &mut Vec<PathBuf>,
) -> Result<(), std::io::Error> {
    let (tx, rx) = std::sync::mpsc::channel();
    let has_error = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    walker.run(|| {
        let tx = tx.clone();
        let has_error = std::sync::Arc::clone(&has_error);
        Box::new(move |entry| {
            if has_error.load(std::sync::atomic::Ordering::Relaxed) {
                return ignore::WalkState::Quit;
            }
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    let is_file = entry
                        .file_type()
                        .map_or_else(|| path.is_file(), |ft| ft.is_file());
                    if is_file && is_kotlin_file(path) {
                        let _ = tx.send(Ok(clean_path(entry.into_path())));
                    }
                    ignore::WalkState::Continue
                }
                Err(err) => {
                    has_error.store(true, std::sync::atomic::Ordering::Relaxed);
                    let _ = tx.send(Err(std::io::Error::other(err.to_string())));
                    ignore::WalkState::Quit
                }
            }
        })
    });
    drop(tx);

    for item in rx {
        target.push(item?);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{clean_path, is_kotlin_file, resolve_target_files};
    use std::path::{Path, PathBuf};

    #[test]
    fn test_is_kotlin_file_extensions() {
        assert!(is_kotlin_file(Path::new("src/Main.kt")));
        assert!(is_kotlin_file(Path::new("build.gradle.kts")));
        assert!(is_kotlin_file(Path::new("File.KT")));
        assert!(!is_kotlin_file(Path::new("Main.java")));
        assert!(!is_kotlin_file(Path::new("README.md")));
        assert!(!is_kotlin_file(Path::new("src/")));
    }

    #[test]
    fn test_resolve_missing_path_fails() {
        let err = resolve_target_files(&["non_existent_file.kt".to_string()]).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn test_clean_path_strips_current_dir_prefix() {
        assert_eq!(
            clean_path(PathBuf::from("./src/Main.kt")),
            PathBuf::from("src/Main.kt")
        );
        assert_eq!(
            clean_path(PathBuf::from("src/Main.kt")),
            PathBuf::from("src/Main.kt")
        );
    }

    #[test]
    fn test_resolve_target_files_nested_hierarchy_filters_non_kotlin() {
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temp_dir = std::env::temp_dir().join(format!("kost_test_walk_{unique_id}"));
        let sub_dir = temp_dir.join("nested");
        std::fs::create_dir_all(&sub_dir).unwrap();

        let kt_file1 = temp_dir.join("Root.kt");
        let kts_file = sub_dir.join("build.gradle.kts");
        let kt_file2 = sub_dir.join("Util.kt");
        let java_file = sub_dir.join("Other.java");
        let md_file = temp_dir.join("README.md");

        std::fs::write(&kt_file1, "fun root() {}").unwrap();
        std::fs::write(&kts_file, "plugins {}").unwrap();
        std::fs::write(&kt_file2, "fun util() {}").unwrap();
        std::fs::write(&java_file, "class Other {}").unwrap();
        std::fs::write(&md_file, "# Readme").unwrap();

        let resolved = resolve_target_files(&[temp_dir.to_string_lossy().to_string()]).unwrap();
        assert_eq!(resolved.len(), 3);
        assert!(resolved.contains(&kt_file1));
        assert!(resolved.contains(&kts_file));
        assert!(resolved.contains(&kt_file2));
        assert!(!resolved.contains(&java_file));
        assert!(!resolved.contains(&md_file));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

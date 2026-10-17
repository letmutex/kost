//! Upward directory resolution, caching, and property merging for .editorconfig files.

use ahash::AHashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use crate::editorconfig::model::ResolvedEditorConfig;
use crate::editorconfig::parser::EditorConfigFile;
use crate::editorconfig::property::apply_file_properties;

/// Thread-safe cached resolver for .editorconfig files across directory hierarchies.
pub struct EditorConfigResolver {
    cache: RwLock<AHashMap<PathBuf, Option<Arc<EditorConfigFile>>>>,
    fallback_config: Option<(PathBuf, Arc<EditorConfigFile>)>,
    cwd: PathBuf,
}

impl Default for EditorConfigResolver {
    fn default() -> Self {
        Self {
            cache: RwLock::default(),
            fallback_config: None,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }
}

impl EditorConfigResolver {
    /// Creates a resolver without fallback defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a resolver with an optional fallback `.editorconfig` file path.
    pub fn new_with_fallback(fallback_path: Option<&str>) -> Self {
        let mut resolver = Self::new();
        if let Some(path_str) = fallback_path {
            let path = Path::new(path_str);
            let fallback_dir = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            if let Ok(content) = std::fs::read_to_string(path)
                && let Ok(parsed) = EditorConfigFile::parse(&content)
            {
                resolver.fallback_config = Some((fallback_dir, Arc::new(parsed)));
            }
        }
        resolver
    }

    /// Resolves all applicable EditorConfig properties for the specified file path.
    pub fn resolve_for_path(&self, file_path: &Path) -> ResolvedEditorConfig {
        let mut resolved = ResolvedEditorConfig::new();
        let abs_file = to_absolute_path(file_path, &self.cwd);

        // 1. Apply fallback defaults first if available
        if let Some((fallback_dir, fallback)) = &self.fallback_config {
            let relative_path = compute_relative_path(fallback_dir, &abs_file, &self.cwd);
            apply_file_properties(fallback, &relative_path, &mut resolved);
        }

        // 2. Collect directory hierarchy up to filesystem root or root = true
        let chain = self.collect_hierarchy(&abs_file);

        // 3. Apply matching files in top-down order (root-most first)
        for (dir, config) in chain.iter().rev() {
            let relative_path = compute_relative_path(dir, &abs_file, &self.cwd);
            apply_file_properties(config, &relative_path, &mut resolved);
        }

        resolved
    }

    fn collect_hierarchy(&self, abs_file: &Path) -> Vec<(PathBuf, Arc<EditorConfigFile>)> {
        let mut chain = Vec::new();
        let Some(parent_dir) = abs_file.parent() else {
            return chain;
        };

        let mut current_dir = Some(parent_dir);
        while let Some(dir) = current_dir {
            if let Some(config) = self.get_or_load_dir_config(dir) {
                let is_root = config.is_root;
                chain.push((dir.to_path_buf(), config));
                if is_root {
                    break;
                }
            }
            current_dir = dir.parent();
        }

        chain
    }

    fn get_or_load_dir_config(&self, dir: &Path) -> Option<Arc<EditorConfigFile>> {
        if let Ok(read_guard) = self.cache.read()
            && let Some(cached) = read_guard.get(dir)
        {
            return cached.clone();
        }

        let editorconfig_path = dir.join(".editorconfig");
        let parsed = if editorconfig_path.is_file() {
            match std::fs::read_to_string(&editorconfig_path) {
                Ok(content) => EditorConfigFile::parse(&content).ok().map(Arc::new),
                Err(_) => None,
            }
        } else {
            None
        };

        if let Ok(mut write_guard) = self.cache.write() {
            write_guard.insert(dir.to_path_buf(), parsed.clone());
        }

        parsed
    }
}

fn to_absolute_path(path: &Path, cwd: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn compute_relative_path(base_dir: &Path, abs_file: &Path, cwd: &Path) -> String {
    let abs_base = to_absolute_path(base_dir, cwd);

    let rel_str = match abs_file.strip_prefix(&abs_base) {
        Ok(stripped) => stripped.to_string_lossy().replace('\\', "/"),
        Err(_) => abs_file.to_string_lossy().replace('\\', "/"),
    };
    let clean_rel = rel_str.strip_prefix("./").unwrap_or(&rel_str);
    clean_rel.to_string()
}

#[cfg(test)]
mod tests {
    use crate::editorconfig::model::{LineLengthSetting, ResolvedEditorConfig};
    use crate::editorconfig::parser::EditorConfigFile;
    use crate::editorconfig::property::apply_file_properties;

    #[test]
    fn test_editorconfig_resolves_child_override() {
        let parent_content = r#"
root = true
[*]
indent_size = 4
max_line_length = 120
"#;
        let child_content = r#"
[*.kt]
indent_size = 2
ktlint_standard_no-semi = disabled
"#;
        let parent_file = EditorConfigFile::parse(parent_content).unwrap();
        let child_file = EditorConfigFile::parse(child_content).unwrap();

        let mut resolved = ResolvedEditorConfig::new();
        apply_file_properties(&parent_file, "src/Main.kt", &mut resolved);
        apply_file_properties(&child_file, "src/Main.kt", &mut resolved);

        assert_eq!(resolved.indent_size, Some(2));
        assert_eq!(
            resolved.max_line_length,
            Some(LineLengthSetting::Value(120))
        );
        assert!(!resolved.is_rule_enabled("standard:no-semi", true));
        assert!(resolved.is_rule_enabled("standard:final-newline", true));
    }

    #[test]
    fn test_editorconfig_normalizes_underscore_and_hyphen_rule_keys() {
        let content = "[*.kt]\nktlint_standard_no_semi = disabled\n";
        let file = EditorConfigFile::parse(content).unwrap();
        let mut resolved = ResolvedEditorConfig::new();
        apply_file_properties(&file, "src/Main.kt", &mut resolved);

        assert!(!resolved.is_rule_enabled("standard:no-semi", true));
        assert!(!resolved.is_rule_enabled("standard:no_semi", true));
    }

    #[test]
    fn test_editorconfig_unsets_rule_toggle_and_updates_tab_indent() {
        let content =
            "[*.kt]\nindent_size = tab\ntab_width = 8\nktlint_standard_no_semi = disabled\n";
        let unset_content = "[*.kt]\nktlint_standard_no_semi = unset\n";
        let file = EditorConfigFile::parse(content).unwrap();
        let unset_file = EditorConfigFile::parse(unset_content).unwrap();
        let mut resolved = ResolvedEditorConfig::new();
        apply_file_properties(&file, "src/Main.kt", &mut resolved);
        assert_eq!(resolved.indent_size, Some(8));
        assert_eq!(resolved.tab_width, Some(8));
        assert!(!resolved.is_rule_enabled("standard:no-semi", true));

        apply_file_properties(&unset_file, "src/Main.kt", &mut resolved);
        assert!(resolved.is_rule_enabled("standard:no-semi", true));
    }
}

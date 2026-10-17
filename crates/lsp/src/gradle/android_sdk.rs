use std::path::{Path, PathBuf};

use crate::gradle::config::{GradleConfig, dirs_home};
use crate::gradle::version::ArtifactVersion;

/// Resolved Android SDK Platform information.
#[derive(Debug, Clone)]
pub struct AndroidSdkPlatform {
    pub api_level: u32,
    pub platform_name: String,
    pub android_jar: PathBuf,
    pub sources_dir: Option<PathBuf>,
}

pub struct AndroidSdkResolver;

impl AndroidSdkResolver {
    /// Locate the Android SDK root directory.
    pub fn find_sdk_root(workspace_root: Option<&Path>, config: &GradleConfig) -> Option<PathBuf> {
        // Explicitly configured path in GradleConfig
        if let Some(ref root) = config.android_sdk_root
            && root.exists()
        {
            return Some(root.clone());
        }

        // local.properties in workspace root
        if let Some(ws) = workspace_root {
            let local_props = ws.join("local.properties");
            if let Ok(content) = std::fs::read_to_string(&local_props)
                && let Some(path) = parse_sdk_dir_from_local_properties(&content)
            {
                let resolved = if path.is_relative() {
                    ws.join(&path)
                } else {
                    path
                };
                if resolved.exists() {
                    return Some(resolved);
                }
            }
        }

        // Environment variables
        for env_var in &["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
            if let Ok(val) = std::env::var(env_var) {
                let p = PathBuf::from(val);
                if p.exists() {
                    return Some(p);
                }
            }
        }

        // Standard default platform paths
        get_default_sdk_paths()
            .into_iter()
            .find(|default_path| default_path.exists())
    }

    /// Resolve the best Android SDK platform (highest version or matching compile_sdk).
    pub fn resolve_platform(
        sdk_root: &Path,
        preferred_api: Option<u32>,
    ) -> Option<AndroidSdkPlatform> {
        let platforms_dir = sdk_root.join("platforms");
        if !platforms_dir.is_dir() {
            return None;
        }

        let mut available_platforms = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&platforms_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && let Some(dir_name) = path.file_name().and_then(|n| n.to_str())
                {
                    let android_jar = path.join("android.jar");
                    if android_jar.is_file() {
                        let version = ArtifactVersion::parse(dir_name);
                        let api_level = extract_api_level(dir_name);
                        available_platforms.push((
                            api_level,
                            version,
                            dir_name.to_string(),
                            path,
                            android_jar,
                        ));
                    }
                }
            }
        }

        if available_platforms.is_empty() {
            return None;
        }

        // If preferred_api is requested, try exact match first
        if let Some(target_api) = preferred_api
            && let Some((api, _, name, _, jar)) = available_platforms
                .iter()
                .find(|(api, _, _, _, _)| *api == target_api)
        {
            let sources_dir = find_sources_dir(sdk_root, *api, name);
            return Some(AndroidSdkPlatform {
                api_level: *api,
                platform_name: name.clone(),
                android_jar: jar.clone(),
                sources_dir,
            });
        }

        // Otherwise pick the highest version
        available_platforms.sort_by(|a, b| b.1.cmp(&a.1));

        let (api, _, name, _, jar) = available_platforms.into_iter().next()?;
        let sources_dir = find_sources_dir(sdk_root, api, &name);

        Some(AndroidSdkPlatform {
            api_level: api,
            platform_name: name,
            android_jar: jar,
            sources_dir,
        })
    }
}

fn find_sources_dir(sdk_root: &Path, api_level: u32, platform_name: &str) -> Option<PathBuf> {
    let sources_base = sdk_root.join("sources");
    if !sources_base.is_dir() {
        return None;
    }

    let exact = sources_base.join(platform_name);
    if exact.is_dir() {
        return Some(exact);
    }

    let api_name = crate::concat_str!("android-", api_level.to_string());
    let api_dir = sources_base.join(api_name);
    if api_dir.is_dir() {
        return Some(api_dir);
    }

    None
}

fn extract_api_level(platform_name: &str) -> u32 {
    let clean = platform_name
        .trim_start_matches("android-")
        .trim_start_matches("android_");
    clean.parse::<u32>().unwrap_or(0)
}

pub fn parse_sdk_dir_from_local_properties(content: &str) -> Option<PathBuf> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if let Some(idx) = trimmed.find('=') {
            let key = trimmed[..idx].trim();
            if key == "sdk.dir" {
                let val = trimmed[idx + 1..].trim();
                // Replace escaped colons and backslashes (e.g. C\:\\Users\\... -> C:\Users\...)
                let unescaped = val.replace("\\:", ":").replace("\\\\", "\\");
                return Some(PathBuf::from(unescaped));
            }
        }
    }
    None
}

fn get_default_sdk_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs_home() {
        // Windows
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            paths.push(PathBuf::from(local_app_data).join("Android").join("Sdk"));
        }
        paths.push(
            home.join("AppData")
                .join("Local")
                .join("Android")
                .join("Sdk"),
        );
        // macOS
        paths.push(home.join("Library").join("Android").join("sdk"));
        // Linux
        paths.push(home.join("Android").join("Sdk"));
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::parse_sdk_dir_from_local_properties;
    use std::path::PathBuf;

    #[test]
    fn test_parse_local_properties() {
        let content = r#"
## This file is automatically generated by Android Studio.
# Do not modify this file -- YOUR CHANGES WILL BE ERASED!
sdk.dir=C\:\\Users\\Alice\\AppData\\Local\\Android\\Sdk
"#;
        let path = parse_sdk_dir_from_local_properties(content).unwrap();
        assert_eq!(
            path,
            PathBuf::from(r#"C:\Users\Alice\AppData\Local\Android\Sdk"#)
        );
    }
}

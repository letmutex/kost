use ahash::AHashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cache::is_safe_component;
use crate::coordinate::DependencyCoordinate;
use crate::version::ArtifactVersion;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleDescriptor {
    pub coordinate: DependencyCoordinate,
    pub dependencies: Vec<DependencyCoordinate>,
    pub artifacts: Vec<String>,
}

impl ModuleDescriptor {
    pub fn parse_file(path: &Path) -> Option<Self> {
        let bytes = fs::read(path).ok()?;
        Self::parse_bytes(&bytes)
    }

    pub fn parse_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }

        let strings = decode_all_strings(bytes);
        if strings.len() < 3 {
            return None;
        }

        let root_coord = DependencyCoordinate::new(
            strings[0].clone(),
            strings[1].clone(),
            Some(strings[2].as_str()),
        );

        let mut dependencies = Vec::new();
        let mut artifacts = Vec::new();
        let mut seen_artifacts = AHashSet::default();
        let mut seen_dependencies = AHashSet::default();

        for item in &strings {
            if (item.ends_with(".aar") || item.ends_with(".jar"))
                && seen_artifacts.insert(item.as_str())
            {
                artifacts.push(item.clone());
            }
        }

        for window in strings.windows(3) {
            let group = &window[0];
            let artifact_name = &window[1];
            let version = &window[2];

            if is_valid_group(group)
                && is_valid_artifact(artifact_name)
                && is_valid_version(version)
            {
                if group == root_coord.group.as_str() && artifact_name == root_coord.name.as_str() {
                    continue;
                }
                let dep = DependencyCoordinate::new(
                    group.clone(),
                    artifact_name.clone(),
                    Some(version.as_str()),
                );
                if seen_dependencies.insert(dep.clone()) {
                    dependencies.push(dep);
                }
            }
        }

        Some(Self {
            coordinate: root_coord,
            dependencies,
            artifacts,
        })
    }

    pub fn find_in_cache(gradle_user_home: &Path, coord: &DependencyCoordinate) -> Option<Self> {
        let descriptor_path = Self::find_descriptor_file(gradle_user_home, coord)?;
        Self::parse_file(&descriptor_path)
    }

    pub fn discover_metadata_dirs(gradle_user_home: &Path) -> Vec<PathBuf> {
        let caches = gradle_user_home.join("caches").join("modules-2");
        if !caches.is_dir() {
            return Vec::new();
        }

        let mut metadata_dirs = Vec::new();
        if let Ok(entries) = fs::read_dir(&caches) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_dir()
                    && entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with("metadata-"))
                {
                    metadata_dirs.push(entry.path());
                }
            }
        }

        metadata_dirs.sort_by_key(|path| {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            std::cmp::Reverse(parse_metadata_version(name))
        });
        metadata_dirs
    }

    pub fn find_descriptor_file_with_dirs(
        metadata_dirs: &[PathBuf],
        coord: &DependencyCoordinate,
    ) -> Option<PathBuf> {
        if !is_safe_component(coord.group.as_str()) || !is_safe_component(coord.name.as_str()) {
            return None;
        }
        if let Some(ref ver) = coord.version
            && !is_safe_component(ver.raw())
        {
            return None;
        }

        for meta_dir in metadata_dirs {
            let desc_dir = meta_dir
                .join("descriptors")
                .join(coord.group.as_str())
                .join(coord.name.as_str());

            if let Some(ref ver) = coord.version {
                let ver_dir = desc_dir.join(ver.raw());
                if let Some(path) = find_descriptor_under(&ver_dir) {
                    return Some(path);
                }
            } else if let Ok(ver_entries) = fs::read_dir(&desc_dir) {
                let mut version_dirs: Vec<(ArtifactVersion, PathBuf)> = ver_entries
                    .flatten()
                    .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .filter_map(|entry| {
                        let name = entry.file_name();
                        let name_str = name.to_str()?;
                        Some((ArtifactVersion::parse(name_str), entry.path()))
                    })
                    .collect();
                version_dirs.sort_by(|a, b| b.0.cmp(&a.0));
                for (_, version_dir) in version_dirs {
                    if let Some(path) = find_descriptor_under(&version_dir) {
                        return Some(path);
                    }
                }
            }
        }

        None
    }

    pub fn find_in_cache_with_dirs(
        metadata_dirs: &[PathBuf],
        coord: &DependencyCoordinate,
    ) -> Option<Self> {
        let descriptor_path = Self::find_descriptor_file_with_dirs(metadata_dirs, coord)?;
        Self::parse_file(&descriptor_path)
    }

    pub fn find_descriptor_file(
        gradle_user_home: &Path,
        coord: &DependencyCoordinate,
    ) -> Option<PathBuf> {
        let metadata_dirs = Self::discover_metadata_dirs(gradle_user_home);
        Self::find_descriptor_file_with_dirs(&metadata_dirs, coord)
    }

    pub fn create_test_descriptor_bytes(
        group: &str,
        name: &str,
        version: &str,
        dependencies: &[(&str, &str, &str)],
        artifacts: &[&str],
    ) -> Vec<u8> {
        let mut buf = vec![0x02];
        write_kryo_string(&mut buf, group);
        write_kryo_string(&mut buf, name);
        write_kryo_string(&mut buf, version);
        write_kryo_string(&mut buf, "org.gradle.status");
        write_kryo_string(&mut buf, "release");

        for (dep_group, dep_name, dep_ver) in dependencies {
            write_kryo_string(&mut buf, dep_group);
            write_kryo_string(&mut buf, dep_name);
            write_kryo_string(&mut buf, dep_ver);
        }

        for art in artifacts {
            write_kryo_string(&mut buf, art);
        }

        buf
    }
}

fn find_descriptor_under(dir: &Path) -> Option<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        if let Ok(file_type) = entry.file_type()
            && file_type.is_dir()
        {
            let bin = entry.path().join("descriptor.bin");
            if bin.is_file() {
                return Some(bin);
            }
        } else if entry.file_name().to_str() == Some("descriptor.bin") {
            return Some(entry.path());
        }
    }
    None
}

fn decode_all_strings(bytes: &[u8]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut idx = 0;
    while idx < bytes.len() {
        let byte = bytes[idx];
        idx += 1;
        if (0x82..=0xFF).contains(&byte) {
            let len = ((byte & 0x7F) - 1) as usize;
            if len >= 1 && idx + len <= bytes.len() {
                let slice = &bytes[idx..idx + len];
                if slice.iter().all(|ch| (0x20..=0x7E).contains(ch))
                    && let Ok(text) = std::str::from_utf8(slice)
                {
                    strings.push(text.to_string());
                    idx += len;
                }
            }
        }
    }
    strings
}

fn write_kryo_string(buf: &mut Vec<u8>, text: &str) {
    let len = text.len();
    buf.push(((len + 1) as u8) | 0x80);
    buf.extend_from_slice(text.as_bytes());
}

fn is_valid_group(text: &str) -> bool {
    if text.is_empty() || text.starts_with("org.gradle.") {
        return false;
    }
    match text {
        "release" | "debug" | "integration" | "milestone" | "library" | "external" | "sources"
        | "documentation" | "aar" | "jar" => false,
        _ => text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_'),
    }
}

fn is_valid_artifact(text: &str) -> bool {
    if text.is_empty()
        || text.ends_with(".aar")
        || text.ends_with(".jar")
        || text.ends_with(".pom")
        || text.ends_with(".module")
    {
        return false;
    }
    match text {
        "release" | "debug" | "java-api" | "java-runtime" | "library" | "external" => false,
        _ => {
            if text.contains("Publication") || text.contains("Elements") {
                return false;
            }
            text.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
        }
    }
}

fn is_valid_version(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let first = bytes[0];
    if first.is_ascii_digit() {
        bytes[1..]
            .iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_' || b == b'+')
    } else if (first == b'[' || first == b'(') && bytes.len() >= 3 {
        let last = bytes[bytes.len() - 1];
        (last == b']' || last == b')')
            && bytes[1..bytes.len() - 1].iter().all(|&b| {
                b.is_ascii_alphanumeric()
                    || b == b'.'
                    || b == b'-'
                    || b == b'_'
                    || b == b'+'
                    || b == b','
                    || b == b' '
            })
    } else {
        false
    }
}

fn parse_metadata_version(dir_name: &str) -> (u32, u32) {
    let suffix = dir_name.strip_prefix("metadata-").unwrap_or("");
    let mut parts = suffix.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor)
}

use ahash::AHashMap;
use common::concat_str;
use smol_str::SmolStr;
use std::fmt;

use crate::version::ArtifactVersion;

/// Represents a Maven / Gradle dependency coordinate (group:artifact:version).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DependencyCoordinate {
    pub group: SmolStr,
    pub name: SmolStr,
    pub version: Option<ArtifactVersion>,
    pub classifier: Option<SmolStr>,
    pub extension: SmolStr,
}

impl DependencyCoordinate {
    pub fn new(group: impl Into<SmolStr>, name: impl Into<SmolStr>, version: Option<&str>) -> Self {
        Self {
            group: group.into(),
            name: name.into(),
            version: version.map(ArtifactVersion::parse),
            classifier: None,
            extension: SmolStr::new("jar"),
        }
    }

    pub fn with_extension(mut self, ext: impl Into<SmolStr>) -> Self {
        self.extension = ext.into();
        self
    }

    pub fn with_classifier(mut self, classifier: impl Into<SmolStr>) -> Self {
        self.classifier = Some(classifier.into());
        self
    }

    /// Parse a standard Gradle coordinate string e.g. "androidx.core:core-ktx:1.15.0@aar"
    pub fn parse(input: &str) -> Option<Self> {
        let clean = input.trim().trim_matches('"').trim_matches('\'');
        let (main_part, ext) = match clean.find('@') {
            Some(idx) => {
                let raw_ext = clean[idx + 1..].trim();
                if raw_ext.is_empty() {
                    return None;
                }
                (&clean[..idx], SmolStr::new(raw_ext))
            }
            None => (clean, SmolStr::new("jar")),
        };

        let parts: Vec<&str> = main_part.split(':').collect();
        if !(2..=4).contains(&parts.len()) {
            return None;
        }

        let raw_group = parts[0].trim();
        let raw_name = parts[1].trim();
        if raw_group.is_empty()
            || raw_name.is_empty()
            || raw_group.chars().any(char::is_whitespace)
            || raw_name.chars().any(char::is_whitespace)
        {
            return None;
        }

        let group = SmolStr::new(raw_group);
        let name = SmolStr::new(raw_name);
        let version = parts
            .get(2)
            .map(|ver_part| ver_part.trim())
            .filter(|ver_part| {
                !ver_part.is_empty()
                    && !ver_part.contains('$')
                    && !ver_part.contains('(')
                    && !ver_part.contains(')')
            })
            .map(ArtifactVersion::parse);
        let classifier = parts
            .get(3)
            .map(|class_part| class_part.trim())
            .filter(|class_part| !class_part.is_empty())
            .map(SmolStr::new);

        Some(Self {
            group,
            name,
            version,
            classifier,
            extension: ext,
        })
    }

    /// Key for exact deduplication: `(group, name)`.
    pub fn key(&self) -> (SmolStr, SmolStr) {
        (self.group.clone(), self.name.clone())
    }

    /// Check if the coordinate version is omitted or dynamic (e.g. `latest`, `+`).
    pub fn is_version_unspecified(&self) -> bool {
        match &self.version {
            None => true,
            Some(ver) => {
                let raw = ver.raw();
                raw.is_empty()
                    || raw.eq_ignore_ascii_case("latest")
                    || raw.eq_ignore_ascii_case("latest.release")
                    || raw.eq_ignore_ascii_case("latest.integration")
                    || raw == "+"
            }
        }
    }

    pub fn kmp_target(&self) -> KmpTarget {
        KmpTarget::from_artifact_name(&self.name).1
    }

    pub fn kmp_base_name(&self) -> SmolStr {
        KmpTarget::from_artifact_name(&self.name).0
    }

    /// Key for KMP-aware deduplication: `(group, base_name)`.
    pub fn kmp_key(&self) -> (SmolStr, SmolStr) {
        (self.group.clone(), self.kmp_base_name())
    }

    /// Return candidate artifact names in KMP target preference order:
    /// `-desktop` suffix, `-jvm` suffix, then base name without suffix.
    pub fn kmp_candidate_names(&self) -> Vec<String> {
        let (base, _) = KmpTarget::from_artifact_name(&self.name);
        vec![
            concat_str!(base, "-desktop"),
            concat_str!(base, "-jvm"),
            base.to_string(),
        ]
    }

    /// Return candidate artifact names in repository/metadata probing order:
    /// exact name if target-qualified, otherwise `[base, base-jvm, base-desktop]`.
    pub fn kmp_probe_candidates(&self) -> Vec<String> {
        let (base, target) = KmpTarget::from_artifact_name(&self.name);
        if target != KmpTarget::None {
            return vec![self.name.to_string()];
        }
        vec![
            self.name.to_string(),
            concat_str!(base, "-jvm"),
            concat_str!(base, "-desktop"),
        ]
    }

    pub fn artifact_file_name(&self, is_sources: bool) -> String {
        let ver = self
            .version
            .as_ref()
            .map(|v| v.raw())
            .unwrap_or("unspecified");
        if is_sources {
            concat_str!(&self.name, "-", ver, "-sources.jar")
        } else if let Some(ref classifier) = self.classifier {
            concat_str!(&self.name, "-", ver, "-", classifier, ".", &self.extension)
        } else {
            concat_str!(&self.name, "-", ver, ".", &self.extension)
        }
    }

    pub fn pom_file_name(&self) -> String {
        let ver = self
            .version
            .as_ref()
            .map(|v| v.raw())
            .unwrap_or("unspecified");
        concat_str!(&self.name, "-", ver, ".pom")
    }

    pub fn module_file_name(&self) -> String {
        let ver = self
            .version
            .as_ref()
            .map(|v| v.raw())
            .unwrap_or("unspecified");
        concat_str!(&self.name, "-", ver, ".module")
    }
}

/// Kotlin Multiplatform target variant priority:
/// Prefer `-desktop` suffix -> `-jvm` suffix -> no suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KmpTarget {
    None = 0,
    Jvm = 1,
    Desktop = 2,
}

impl KmpTarget {
    pub fn from_artifact_name(name: &str) -> (SmolStr, Self) {
        if let Some(base) = name.strip_suffix("-desktop")
            && !base.is_empty()
        {
            return (SmolStr::new(base), Self::Desktop);
        }
        if let Some(base) = name.strip_suffix("-jvm")
            && !base.is_empty()
        {
            return (SmolStr::new(base), Self::Jvm);
        }
        (SmolStr::new(name), Self::None)
    }

    pub fn suffix(&self) -> &'static str {
        match self {
            Self::Desktop => "-desktop",
            Self::Jvm => "-jvm",
            Self::None => "",
        }
    }
}

impl fmt::Display for DependencyCoordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.group, self.name)?;
        if let Some(ref ver) = self.version {
            write!(f, ":{}", ver.raw())?;
        }
        if let Some(ref classifier) = self.classifier {
            write!(f, ":{}", classifier)?;
        }
        if self.extension != "jar" {
            write!(f, "@{}", self.extension)?;
        }
        Ok(())
    }
}

/// Deduplicate a collection of dependencies by `(group, kmp_base_name)`,
/// picking the newest version, and breaking version ties by preferring
/// `-desktop` suffix -> `-jvm` suffix -> no suffix.
pub fn deduplicate_newest(
    deps: impl IntoIterator<Item = DependencyCoordinate>,
) -> Vec<DependencyCoordinate> {
    let mut map: AHashMap<(SmolStr, SmolStr), DependencyCoordinate> = AHashMap::default();

    for dep in deps {
        let key = dep.kmp_key();
        match map.get_mut(&key) {
            Some(existing) => {
                let should_replace = match (&existing.version, &dep.version) {
                    (Some(ev), Some(dv)) => match dv.cmp(ev) {
                        std::cmp::Ordering::Greater => true,
                        std::cmp::Ordering::Less => false,
                        std::cmp::Ordering::Equal => dep.kmp_target() > existing.kmp_target(),
                    },
                    (None, Some(_)) => true,
                    (Some(_), None) => false,
                    (None, None) => dep.kmp_target() > existing.kmp_target(),
                };
                if should_replace {
                    *existing = dep;
                }
            }
            None => {
                map.insert(key, dep);
            }
        }
    }

    map.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::{DependencyCoordinate, deduplicate_newest};

    #[test]
    fn test_coordinate_parsing_and_file_names() {
        let dep = DependencyCoordinate::parse("androidx.core:core-ktx:1.15.0@aar").unwrap();
        assert_eq!(dep.group, "androidx.core");
        assert_eq!(dep.name, "core-ktx");
        assert_eq!(dep.version.as_ref().unwrap().raw(), "1.15.0");
        assert_eq!(dep.extension, "aar");
        assert_eq!(dep.artifact_file_name(false), "core-ktx-1.15.0.aar");
        assert_eq!(dep.artifact_file_name(true), "core-ktx-1.15.0-sources.jar");
        assert_eq!(dep.pom_file_name(), "core-ktx-1.15.0.pom");
    }

    #[test]
    fn test_deduplicate_keeps_highest_version() {
        let deps = vec![
            DependencyCoordinate::parse("androidx.core:core-ktx:1.12.0").unwrap(),
            DependencyCoordinate::parse("androidx.core:core-ktx:1.15.0").unwrap(),
        ];
        let deduped = deduplicate_newest(deps);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].version.as_ref().unwrap().raw(), "1.15.0");
    }

    #[test]
    fn test_is_version_unspecified() {
        assert!(
            DependencyCoordinate::parse("org.example:foo")
                .unwrap()
                .is_version_unspecified()
        );
        assert!(
            DependencyCoordinate::parse("org.example:foo:")
                .unwrap()
                .is_version_unspecified()
        );
        assert!(
            DependencyCoordinate::parse("org.example:foo:latest")
                .unwrap()
                .is_version_unspecified()
        );
        assert!(
            DependencyCoordinate::parse("org.example:foo:LATEST")
                .unwrap()
                .is_version_unspecified()
        );
        assert!(
            DependencyCoordinate::parse("org.example:foo:latest.release")
                .unwrap()
                .is_version_unspecified()
        );
        assert!(
            DependencyCoordinate::parse("org.example:foo:latest.integration")
                .unwrap()
                .is_version_unspecified()
        );
        assert!(
            DependencyCoordinate::parse("org.example:foo:+")
                .unwrap()
                .is_version_unspecified()
        );

        assert!(
            !DependencyCoordinate::parse("org.example:foo:1.0.0")
                .unwrap()
                .is_version_unspecified()
        );
        assert!(
            !DependencyCoordinate::parse("org.example:foo:1.0.0-alpha")
                .unwrap()
                .is_version_unspecified()
        );
    }
}

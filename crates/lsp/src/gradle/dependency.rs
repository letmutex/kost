use ahash::AHashMap;
use smol_str::SmolStr;
use std::fmt;

use crate::gradle::version::ArtifactVersion;

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
    pub fn parse(s: &str) -> Option<Self> {
        let clean = s.trim().trim_matches('"').trim_matches('\'');
        let (main_part, ext) = match clean.find('@') {
            Some(idx) => (&clean[..idx], SmolStr::new(&clean[idx + 1..])),
            None => (clean, SmolStr::new("jar")),
        };

        let parts: Vec<&str> = main_part.split(':').collect();
        if parts.len() < 2 {
            return None;
        }

        let group = SmolStr::new(parts[0].trim());
        let name = SmolStr::new(parts[1].trim());
        let version = parts.get(2).map(|v| ArtifactVersion::parse(v.trim()));
        let classifier = parts.get(3).map(|c| SmolStr::new(c.trim()));

        Some(Self {
            group,
            name,
            version,
            classifier,
            extension: ext,
        })
    }

    /// Key for deduplication: (group, name)
    pub fn key(&self) -> (SmolStr, SmolStr) {
        (self.group.clone(), self.name.clone())
    }

    pub fn kmp_target(&self) -> KmpTarget {
        KmpTarget::from_artifact_name(&self.name).1
    }

    pub fn kmp_base_name(&self) -> SmolStr {
        KmpTarget::from_artifact_name(&self.name).0
    }

    /// Key for KMP-aware deduplication: (group, base_name)
    pub fn kmp_key(&self) -> (SmolStr, SmolStr) {
        (self.group.clone(), self.kmp_base_name())
    }

    /// Return candidate artifact names in order of preference:
    /// desktop suffix, jvm suffix, then no suffix.
    pub fn kmp_candidate_names(&self) -> Vec<String> {
        let (base, _) = KmpTarget::from_artifact_name(&self.name);
        vec![
            crate::concat_str!(base, "-desktop"),
            crate::concat_str!(base, "-jvm"),
            base.to_string(),
        ]
    }

    /// Candidate file names for matching in repositories or archives
    pub fn artifact_file_name(&self, is_sources: bool) -> String {
        let ver = self
            .version
            .as_ref()
            .map(|v| v.raw())
            .unwrap_or("unspecified");
        if is_sources {
            crate::concat_str!(&self.name, "-", ver, "-sources.jar")
        } else {
            crate::concat_str!(&self.name, "-", ver, ".", &self.extension)
        }
    }
}

/// Kotlin Multiplatform target variant priority:
/// Prefer -desktop suffix -> -jvm suffix -> no suffix.
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
        if let Some(ref v) = self.version {
            write!(f, ":{}", v.raw())?;
        }
        if let Some(ref c) = self.classifier {
            write!(f, ":{}", c)?;
        }
        if self.extension != "jar" {
            write!(f, "@{}", self.extension)?;
        }
        Ok(())
    }
}

/// Deduplicate a collection of dependencies by (group, kmp_base_name),
/// picking the newest version, and breaking version ties by preferring
/// -desktop suffix -> -jvm suffix -> no suffix.
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
    fn test_parse_coordinate() {
        let dep = DependencyCoordinate::parse("androidx.core:core-ktx:1.15.0@aar").unwrap();
        assert_eq!(dep.group, "androidx.core");
        assert_eq!(dep.name, "core-ktx");
        assert_eq!(dep.version.unwrap().raw(), "1.15.0");
        assert_eq!(dep.extension, "aar");
    }

    #[test]
    fn test_deduplicate_newest_version() {
        let deps = vec![
            DependencyCoordinate::parse("androidx.core:core-ktx:1.12.0").unwrap(),
            DependencyCoordinate::parse("androidx.core:core-ktx:1.15.0").unwrap(),
            DependencyCoordinate::parse("com.google.code.gson:gson:2.9.0").unwrap(),
            DependencyCoordinate::parse("com.google.code.gson:gson:2.10.1").unwrap(),
            DependencyCoordinate::parse("com.google.code.gson:gson:2.8.5").unwrap(),
        ];

        let deduped = deduplicate_newest(deps);
        assert_eq!(deduped.len(), 2);

        let core_ktx = deduped.iter().find(|d| d.name == "core-ktx").unwrap();
        assert_eq!(core_ktx.version.as_ref().unwrap().raw(), "1.15.0");

        let gson = deduped.iter().find(|d| d.name == "gson").unwrap();
        assert_eq!(gson.version.as_ref().unwrap().raw(), "2.10.1");
    }
}

use smol_str::SmolStr;
use std::cmp::Ordering;
use std::fmt;

/// Represents a parsed library or SDK version.
/// Supports SemVer, Gradle/Maven pre-release qualifiers, snapshot versions, and Android SDK platforms.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtifactVersion {
    pub raw: SmolStr,
    pub components: Vec<VersionComponent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VersionComponent {
    Number(u64),
    Qualifier(QualifierKind, u64),
    String(SmolStr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QualifierKind {
    Dev = 0,
    Snapshot = 1,
    Alpha = 2,
    Beta = 3,
    Preview = 4,
    Milestone = 5,
    Rc = 6,
    Final = 7,
    Release = 8,
}

impl ArtifactVersion {
    pub fn parse(input: &str) -> Self {
        let trimmed = input.trim().trim_start_matches('v').trim_start_matches('V');

        let clean = if let Some(stripped) = trimmed.strip_prefix("android-") {
            stripped
        } else if let Some(stripped) = trimmed.strip_prefix("android_") {
            stripped
        } else {
            trimmed
        };

        let mut components = Vec::new();
        let mut current_digits = String::new();
        let mut current_letters = String::new();

        for ch in clean.chars() {
            if ch.is_ascii_digit() {
                flush_letters(&mut current_letters, &mut components);
                current_digits.push(ch);
            } else if ch.is_ascii_alphabetic() {
                flush_digits(&mut current_digits, &mut components);
                current_letters.push(ch);
            } else {
                flush_digits(&mut current_digits, &mut components);
                flush_letters(&mut current_letters, &mut components);
            }
        }
        flush_digits(&mut current_digits, &mut components);
        flush_letters(&mut current_letters, &mut components);

        Self {
            raw: SmolStr::new(input),
            components,
        }
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Returns true if this version represents a stable release (contains at least one numeric
    /// component and no pre-release qualifiers such as Dev, Snapshot, Alpha, Beta, Preview, Milestone, Rc).
    pub fn is_stable(&self) -> bool {
        !self.components.is_empty()
            && self
                .components
                .iter()
                .any(|comp| matches!(comp, VersionComponent::Number(_)))
            && !self.components.iter().any(|comp| match comp {
                VersionComponent::Qualifier(kind, _) => *kind < QualifierKind::Final,
                VersionComponent::String(_) => true,
                VersionComponent::Number(_) => false,
            })
    }
}

/// Select the best version from an iterator of version references:
/// 1. Finds the highest stable version among the candidates.
/// 2. If no stable version exists, finds the highest pre-release version.
pub fn select_best_version<'a>(
    versions: impl IntoIterator<Item = &'a ArtifactVersion>,
) -> Option<ArtifactVersion> {
    let list: Vec<&'a ArtifactVersion> = versions.into_iter().collect();
    if let Some(stable) = list
        .iter()
        .filter(|v| v.is_stable())
        .max_by(|a, b| a.cmp(b))
    {
        return Some((*stable).clone());
    }
    list.into_iter().max_by(|a, b| a.cmp(b)).cloned()
}

impl fmt::Display for ArtifactVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

fn flush_letters(letters: &mut String, components: &mut Vec<VersionComponent>) {
    if letters.is_empty() {
        return;
    }
    let lower = letters.to_lowercase();
    let qual = match lower.as_str() {
        "dev" => Some(QualifierKind::Dev),
        "snapshot" => Some(QualifierKind::Snapshot),
        "alpha" | "a" => Some(QualifierKind::Alpha),
        "beta" | "b" => Some(QualifierKind::Beta),
        "preview" => Some(QualifierKind::Preview),
        "milestone" | "m" => Some(QualifierKind::Milestone),
        "rc" | "cr" => Some(QualifierKind::Rc),
        "final" | "ga" | "release" => Some(QualifierKind::Final),
        _ => None,
    };
    if let Some(qual_kind) = qual {
        components.push(VersionComponent::Qualifier(qual_kind, 0));
    } else {
        components.push(VersionComponent::String(SmolStr::new(lower)));
    }
    letters.clear();
}

fn flush_digits(digits: &mut String, components: &mut Vec<VersionComponent>) {
    if digits.is_empty() {
        return;
    }
    if let Ok(num) = digits.parse::<u64>() {
        if let Some(VersionComponent::Qualifier(_qual, count)) = components.last_mut()
            && *count == 0
        {
            *count = num;
            digits.clear();
            return;
        }
        components.push(VersionComponent::Number(num));
    } else {
        components.push(VersionComponent::String(SmolStr::new(digits.as_str())));
    }
    digits.clear();
}

impl PartialOrd for ArtifactVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ArtifactVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let max_len = self.components.len().max(other.components.len());
        for idx in 0..max_len {
            let left = self.components.get(idx);
            let right = other.components.get(idx);

            match (left, right) {
                (Some(left_comp), Some(right_comp)) => {
                    match compare_components(left_comp, right_comp) {
                        Ordering::Equal => continue,
                        non_eq => return non_eq,
                    }
                }
                (Some(VersionComponent::Qualifier(..) | VersionComponent::String(_)), None) => {
                    return Ordering::Less;
                }
                (None, Some(VersionComponent::Qualifier(..) | VersionComponent::String(_))) => {
                    return Ordering::Greater;
                }
                (Some(VersionComponent::Number(num)), None) => {
                    if *num != 0 {
                        return Ordering::Greater;
                    }
                }
                (None, Some(VersionComponent::Number(num))) => {
                    if *num != 0 {
                        return Ordering::Less;
                    }
                }
                _ => return Ordering::Equal,
            }
        }
        self.raw.cmp(&other.raw)
    }
}

fn compare_components(left: &VersionComponent, right: &VersionComponent) -> Ordering {
    match (left, right) {
        (VersionComponent::Number(a), VersionComponent::Number(b)) => a.cmp(b),
        (VersionComponent::Qualifier(qa, na), VersionComponent::Qualifier(qb, nb)) => {
            match qa.cmp(qb) {
                Ordering::Equal => na.cmp(nb),
                ord => ord,
            }
        }
        (VersionComponent::String(a), VersionComponent::String(b)) => a.cmp(b),
        (VersionComponent::Number(_), VersionComponent::Qualifier(..)) => Ordering::Greater,
        (VersionComponent::Qualifier(..), VersionComponent::Number(_)) => Ordering::Less,
        (VersionComponent::Number(_), VersionComponent::String(_)) => Ordering::Greater,
        (VersionComponent::String(_), VersionComponent::Number(_)) => Ordering::Less,
        (VersionComponent::Qualifier(..), VersionComponent::String(_)) => Ordering::Greater,
        (VersionComponent::String(_), VersionComponent::Qualifier(..)) => Ordering::Less,
    }
}

#[cfg(test)]
mod tests {
    use super::{ArtifactVersion, select_best_version};

    #[test]
    fn version_ordering_precedence() {
        let v1 = ArtifactVersion::parse("1.12.0");
        let v2 = ArtifactVersion::parse("1.15.0");
        assert!(v2 > v1);

        let v_alpha = ArtifactVersion::parse("1.15.0-alpha01");
        let v_beta = ArtifactVersion::parse("1.15.0-beta02");
        let v_rc = ArtifactVersion::parse("1.15.0-rc01");
        let v_final = ArtifactVersion::parse("1.15.0");
        let v_patch = ArtifactVersion::parse("1.15.1");

        assert!(v_beta > v_alpha);
        assert!(v_rc > v_beta);
        assert!(v_final > v_rc);
        assert!(v_patch > v_final);

        let v_custom = ArtifactVersion::parse("1.15.0-custom");
        assert!(v_alpha > v_custom);
        assert!(v_final > v_custom);
    }

    #[test]
    fn large_digit_version_parsing_without_overflow() {
        let v1 = ArtifactVersion::parse("1.0.0-20241030123456789012345");
        let v2 = ArtifactVersion::parse("1.0.0-20241030123456789012346");
        assert!(v2 > v1);
    }

    #[test]
    fn version_stability_classification() {
        assert!(ArtifactVersion::parse("1.15.0").is_stable());
        assert!(ArtifactVersion::parse("1.15.0-final").is_stable());
        assert!(ArtifactVersion::parse("1.15.0-release").is_stable());
        assert!(ArtifactVersion::parse("1.15.0-ga").is_stable());
        assert!(ArtifactVersion::parse("2.0").is_stable());

        assert!(!ArtifactVersion::parse("1.15.0-alpha01").is_stable());
        assert!(!ArtifactVersion::parse("1.15.0-beta02").is_stable());
        assert!(!ArtifactVersion::parse("1.15.0-rc01").is_stable());
        assert!(!ArtifactVersion::parse("1.15.0-SNAPSHOT").is_stable());
        assert!(!ArtifactVersion::parse("1.15.0-dev").is_stable());
        assert!(!ArtifactVersion::parse("1.15.0-preview").is_stable());
        assert!(!ArtifactVersion::parse("1.15.0-custom").is_stable());
        assert!(!ArtifactVersion::parse("").is_stable());
        assert!(!ArtifactVersion::parse("---").is_stable());
        assert!(!ArtifactVersion::parse("release").is_stable());
    }

    #[test]
    fn select_best_version_prefers_stable() {
        let versions = vec![
            ArtifactVersion::parse("1.2.0"),
            ArtifactVersion::parse("1.3.0-rc01"),
            ArtifactVersion::parse("1.1.0"),
        ];
        let best = select_best_version(&versions).unwrap();
        assert_eq!(best.raw(), "1.2.0");
    }

    #[test]
    fn select_best_version_prerelease_fallback() {
        let versions = vec![
            ArtifactVersion::parse("0.1.0-alpha01"),
            ArtifactVersion::parse("0.2.0-beta02"),
        ];
        let best = select_best_version(&versions).unwrap();
        assert_eq!(best.raw(), "0.2.0-beta02");
    }
}

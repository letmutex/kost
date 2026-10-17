use smol_str::SmolStr;
use std::cmp::Ordering;

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

        // Handle android platform prefix e.g. android-35 -> 35
        let clean = if let Some(s) = trimmed.strip_prefix("android-") {
            s
        } else if let Some(s) = trimmed.strip_prefix("android_") {
            s
        } else {
            trimmed
        };

        let mut components = Vec::new();
        let mut current_digits = String::new();
        let mut current_letters = String::new();

        let flush_letters = |letters: &mut String, components: &mut Vec<VersionComponent>| {
            if !letters.is_empty() {
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
                if let Some(q) = qual {
                    components.push(VersionComponent::Qualifier(q, 0));
                } else {
                    components.push(VersionComponent::String(SmolStr::new(lower)));
                }
                letters.clear();
            }
        };

        let flush_digits = |digits: &mut String, components: &mut Vec<VersionComponent>| {
            if !digits.is_empty() {
                if let Ok(num) = digits.parse::<u64>() {
                    // Check if the previous component was a qualifier without a number (e.g. alpha + 01 -> Qualifier(Alpha, 1))
                    if let Some(VersionComponent::Qualifier(_q, n)) = components.last_mut()
                        && *n == 0
                    {
                        *n = num;
                        digits.clear();
                        return;
                    }
                    components.push(VersionComponent::Number(num));
                }
                digits.clear();
            }
        };

        for ch in clean.chars() {
            if ch.is_ascii_digit() {
                flush_letters(&mut current_letters, &mut components);
                current_digits.push(ch);
            } else if ch.is_ascii_alphabetic() {
                flush_digits(&mut current_digits, &mut components);
                current_letters.push(ch);
            } else {
                // Delimiter e.g. '.', '-', '_', '+'
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
}

impl PartialOrd for ArtifactVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ArtifactVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        let max_len = self.components.len().max(other.components.len());
        for i in 0..max_len {
            let left = self.components.get(i);
            let right = other.components.get(i);

            match (left, right) {
                (Some(l), Some(r)) => match compare_components(l, r) {
                    Ordering::Equal => continue,
                    non_eq => return non_eq,
                },
                // If one side ended with a pre-release qualifier (like -alpha, -beta, -rc, -snapshot),
                // the full release (the shorter one) is GREATER.
                // e.g. 1.0.0 is greater than 1.0.0-rc1
                (Some(VersionComponent::Qualifier(..)), None) => return Ordering::Less,
                (None, Some(VersionComponent::Qualifier(..))) => return Ordering::Greater,
                // Extra numeric zero or non-qualifier component (e.g. 1.0.0 vs 1.0)
                (Some(VersionComponent::Number(n)), None) => {
                    if *n != 0 {
                        return Ordering::Greater;
                    }
                }
                (None, Some(VersionComponent::Number(n))) => {
                    if *n != 0 {
                        return Ordering::Less;
                    }
                }
                (Some(_), None) => return Ordering::Greater,
                (None, Some(_)) => return Ordering::Less,
                (None, None) => return Ordering::Equal,
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
    use super::ArtifactVersion;

    #[test]
    fn test_version_order() {
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
    }

    #[test]
    fn test_android_platform_version() {
        let p33 = ArtifactVersion::parse("android-33");
        let p34 = ArtifactVersion::parse("android-34");
        let p35 = ArtifactVersion::parse("android-35");
        assert!(p35 > p34);
        assert!(p34 > p33);
    }

    #[test]
    fn test_kotlin_version() {
        let k1 = ArtifactVersion::parse("1.9.24");
        let k2 = ArtifactVersion::parse("2.0.0");
        let k3 = ArtifactVersion::parse("2.0.20-RC");
        let k4 = ArtifactVersion::parse("2.0.20");
        assert!(k2 > k1);
        assert!(k3 > k2);
        assert!(k4 > k3);
    }
}

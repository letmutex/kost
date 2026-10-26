//! String concatenation utilities avoiding unnecessary format! allocations.

use smol_str::SmolStr;

/// Helper function to convert any string-like reference into `&str`.
#[inline(always)]
pub fn as_str<T: ?Sized + AsRef<str>>(s: &T) -> &str {
    s.as_ref()
}

/// Concatenates multiple string-like slices into a single heap `String`.
pub fn concat_str_slice(parts: &[&str]) -> String {
    let total_len: usize = parts.iter().map(|s| s.len()).sum();
    let mut result = String::with_capacity(total_len);
    for part in parts {
        result.push_str(part);
    }
    result
}

/// Concatenates multiple string-like slices into a `SmolStr`.
pub fn concat_smol_str_slice(parts: &[&str]) -> SmolStr {
    let total_len: usize = parts.iter().map(|s| s.len()).sum();
    if total_len <= 22 {
        let mut buf = [0u8; 22];
        let mut offset = 0;
        for s in parts {
            let bytes = s.as_bytes();
            buf[offset..offset + bytes.len()].copy_from_slice(bytes);
            offset += bytes.len();
        }
        let s = match std::str::from_utf8(&buf[..total_len]) {
            Ok(valid) => valid,
            Err(_) => unreachable!("Concatenation of valid UTF-8 slices is always valid UTF-8"),
        };
        SmolStr::new(s)
    } else {
        let mut s = String::with_capacity(total_len);
        for part in parts {
            s.push_str(part);
        }
        SmolStr::from(s)
    }
}

/// Concatenates string-like arguments into a `String` with a single allocation.
#[macro_export]
macro_rules! concat_str {
    ($($part:expr),* $(,)?) => {
        $crate::util::concat_str_slice(&[$($crate::util::as_str(&$part)),*])
    };
}

/// Concatenates string-like arguments into a `SmolStr`.
#[macro_export]
macro_rules! concat_smol_str {
    ($($part:expr),* $(,)?) => {
        $crate::util::concat_smol_str_slice(&[$($crate::util::as_str(&$part)),*])
    };
}

/// Formats an elapsed duration into a human-readable string (e.g. "12ms", "1.23s", "<1ms").
pub fn format_elapsed(duration: std::time::Duration) -> String {
    let millis = duration.as_millis();
    if millis < 1 {
        "<1ms".to_string()
    } else if millis < 1000 {
        concat_str!(&millis.to_string(), "ms")
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}

/// Validates that a string is valid kebab-case (e.g. "no-semi", "long-function-1").
pub fn is_valid_kebab_case(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    for part in s.split('-') {
        if part.is_empty()
            || !part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use smol_str::SmolStr;

    #[test]
    fn test_concat_str_with_various_string_types() {
        let prefix: &str = "com";
        let dot = String::from(".");
        let name: SmolStr = SmolStr::new("example");
        let result = concat_str!(prefix, dot, name, ".", "User");
        assert_eq!(result, "com.example.User");
    }

    #[test]
    fn test_concat_str_empty_and_single_part() {
        let empty = concat_str!();
        assert_eq!(empty, "");

        let single = concat_str!("hello");
        assert_eq!(single, "hello");

        let trailing = concat_str!("a", "b",);
        assert_eq!(trailing, "ab");
    }

    #[test]
    fn test_concat_smol_str_inline_capacity_boundary() {
        // Exactly 22 bytes (max inline storage on 64-bit systems)
        let part1 = "12345678901";
        let part2 = "12345678901";
        let res22 = concat_smol_str!(part1, part2);
        assert_eq!(res22.len(), 22);
        assert_eq!(res22.as_str(), "1234567890112345678901");

        // 23 bytes (exceeds 22 bytes, uses heap storage)
        let part3 = "123456789012";
        let res23 = concat_smol_str!(part1, part3);
        assert_eq!(res23.len(), 23);
        assert_eq!(res23.as_str(), "12345678901123456789012");
    }

    #[test]
    fn test_concat_smol_str_heap_fallback_long_string() {
        let long_pkg = "androidx.compose.foundation.layout";
        let sep = ".";
        let class_name = "BoxScopeInstance";
        let result = concat_smol_str!(long_pkg, sep, class_name);
        assert_eq!(
            result.as_str(),
            "androidx.compose.foundation.layout.BoxScopeInstance"
        );
        assert!(result.len() > 22);
    }

    #[test]
    fn test_concat_smol_str_with_mixed_string_types() {
        let pkg: &str = "kotlin";
        let mid = String::from(".collections.");
        let cls: SmolStr = SmolStr::new("List");
        let result = concat_smol_str!(pkg, mid, cls);
        assert_eq!(result.as_str(), "kotlin.collections.List");
    }

    #[test]
    fn test_is_valid_kebab_case() {
        use super::is_valid_kebab_case;

        assert!(is_valid_kebab_case("no-semi"));
        assert!(is_valid_kebab_case("long-function-1"));
        assert!(is_valid_kebab_case("rule"));
        assert!(is_valid_kebab_case("rule-123"));

        assert!(!is_valid_kebab_case(""));
        assert!(!is_valid_kebab_case("-leading"));
        assert!(!is_valid_kebab_case("trailing-"));
        assert!(!is_valid_kebab_case("double--hyphen"));
        assert!(!is_valid_kebab_case("Upper-Case"));
        assert!(!is_valid_kebab_case("snake_case"));
        assert!(!is_valid_kebab_case("special!char"));
    }
}

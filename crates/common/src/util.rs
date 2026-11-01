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
        let centis = (duration.as_secs_f64() * 100.0).round() as u64;
        let secs = centis / 100;
        let frac = centis % 100;
        let pad = if frac < 10 { "0" } else { "" };
        concat_str!(&secs.to_string(), ".", pad, &frac.to_string(), "s")
    }
}

/// Format byte counts into human-readable strings (B, KiB, MiB, GiB).
pub fn format_bytes(bytes: u64) -> SmolStr {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;

    if bytes >= GIB {
        let value = (bytes as f64 / GIB as f64 * 100.0).round() / 100.0;
        concat_smol_str!(&value.to_string(), " GiB")
    } else if bytes >= MIB {
        let value = (bytes as f64 / MIB as f64 * 100.0).round() / 100.0;
        concat_smol_str!(&value.to_string(), " MiB")
    } else if bytes >= KIB {
        let value = (bytes as f64 / KIB as f64 * 100.0).round() / 100.0;
        concat_smol_str!(&value.to_string(), " KiB")
    } else {
        concat_smol_str!(&bytes.to_string(), " B")
    }
}

/// Format a fixed-width visual progress bar (e.g. "[=====>    ]").
pub fn format_progress_bar(downloaded: u64, total: u64, width: usize) -> String {
    if total == 0 || width == 0 {
        return String::new();
    }
    let ratio = (downloaded as f64 / total as f64).clamp(0.0, 1.0);
    let filled = ((width as f64) * ratio).round() as usize;
    let mut bar = String::with_capacity(width + 2);
    bar.push('[');
    if filled >= width {
        for _ in 0..width {
            bar.push('=');
        }
    } else {
        for _ in 0..filled {
            bar.push('=');
        }
        bar.push('>');
        for _ in (filled + 1)..width {
            bar.push(' ');
        }
    }
    bar.push(']');
    bar
}

/// Truncates a label to fit within max_len columns, preserving prefix and suffix with an ellipsis.
pub fn truncate_label(label: &str, max_len: usize) -> SmolStr {
    let count = label.chars().count();
    if count <= max_len || max_len < 8 {
        return SmolStr::new(label);
    }
    let keep = (max_len - 3) / 2;
    let prefix_end = label
        .char_indices()
        .nth(keep)
        .map(|(idx, _)| idx)
        .unwrap_or(label.len());
    let suffix_start = label
        .char_indices()
        .nth(count - keep)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    concat_smol_str!(&label[..prefix_end], "...", &label[suffix_start..])
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

pub use common::util::{as_str, concat_smol_str_slice, concat_str_slice};

/// Efficiently concatenates multiple string-like arguments into a `String`.
///
/// Exactly one heap allocation is made with the exact capacity needed.
///
/// # Examples
/// ```
/// use lsp::concat_str;
///
/// let s = concat_str!("com.example", ".", "User");
/// assert_eq!(s, "com.example.User");
/// ```
#[macro_export]
macro_rules! concat_str {
    ($($part:expr),* $(,)?) => {
        $crate::util::concat_str_slice(&[$($crate::util::as_str(&$part)),*])
    };
}

/// Efficiently concatenates multiple string-like arguments into a `SmolStr`.
///
/// If the combined length is 22 bytes or fewer, zero heap allocations are performed
/// and the result is stored entirely inline on the stack.
///
/// # Examples
/// ```
/// use lsp::concat_smol_str;
///
/// let s = concat_smol_str!("com.example", ".", "User");
/// assert_eq!(s.as_str(), "com.example.User");
/// ```
#[macro_export]
macro_rules! concat_smol_str {
    ($($part:expr),* $(,)?) => {
        $crate::util::concat_smol_str_slice(&[$($crate::util::as_str(&$part)),*])
    };
}

pub mod archive;

pub use archive::read_jar_url_content;
pub use concat_smol_str;
pub use concat_str;

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
}

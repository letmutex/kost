//! EditorConfig glob pattern compilation into regex.

use regex::Regex;

/// Compiles an EditorConfig glob pattern string into an executable regular expression.
pub fn compile_glob(pattern: &str) -> Result<Regex, String> {
    let has_slash = pattern.contains('/');
    let clean_pattern = pattern.strip_prefix('/').unwrap_or(pattern);

    let mut regex_str = String::with_capacity(clean_pattern.len() * 2 + 16);
    regex_str.push('^');
    if !has_slash {
        regex_str.push_str("(?:.*/)?");
    }

    let chars: Vec<char> = clean_pattern.chars().collect();
    translate_glob_tokens(&chars, &mut regex_str)?;
    regex_str.push('$');

    Regex::new(&regex_str).map_err(|err| err.to_string())
}

fn translate_glob_tokens(chars: &[char], out: &mut String) -> Result<(), String> {
    let mut index = 0;
    let len = chars.len();

    while index < len {
        if chars[index..].starts_with(&['/', '*', '*', '/']) {
            out.push_str("(?:/|/.*/)");
            index += 4;
            continue;
        }
        if index == 0 && chars.starts_with(&['*', '*', '/']) {
            out.push_str("(?:.*/)?");
            index += 3;
            continue;
        }
        if index + 3 == len && chars[index..].starts_with(&['/', '*', '*']) {
            out.push_str("(?:/.*)?");
            index += 3;
            continue;
        }

        match chars[index] {
            '*' => {
                if index + 1 < len && chars[index + 1] == '*' {
                    out.push_str(".*");
                    index += 2;
                } else {
                    out.push_str("[^/]*");
                    index += 1;
                }
            }
            '?' => {
                out.push_str("[^/]");
                index += 1;
            }
            '\\' => {
                index = append_escaped_char(chars, index, out);
            }
            '[' => {
                index = append_character_class(chars, index, out);
            }
            '{' => {
                index = append_choice_or_range(chars, index, out)?;
            }
            '.' | '(' | ')' | '+' | '|' | '^' | '$' => {
                out.push('\\');
                out.push(chars[index]);
                index += 1;
            }
            other => {
                out.push(other);
                index += 1;
            }
        }
    }

    Ok(())
}

fn append_escaped_char(chars: &[char], index: usize, out: &mut String) -> usize {
    if index + 1 < chars.len() {
        out.push('\\');
        out.push(chars[index + 1]);
        index + 2
    } else {
        out.push('\\');
        index + 1
    }
}

fn append_character_class(chars: &[char], index: usize, out: &mut String) -> usize {
    let len = chars.len();
    let mut close = index + 1;
    if close < len && (chars[close] == '!' || chars[close] == ']') {
        close += 1;
    }
    while close < len && chars[close] != ']' {
        close += 1;
    }

    if close < len {
        out.push('[');
        let mut k = index + 1;
        if chars[k] == '!' {
            out.push_str("^/");
            k += 1;
        }
        while k < close {
            let c = chars[k];
            if c == '\\' && k + 1 < close {
                out.push('\\');
                out.push(chars[k + 1]);
                k += 2;
            } else {
                out.push(c);
                k += 1;
            }
        }
        out.push(']');
        close + 1
    } else {
        out.push_str("\\[");
        index + 1
    }
}

fn append_choice_or_range(chars: &[char], index: usize, out: &mut String) -> Result<usize, String> {
    let len = chars.len();
    let mut depth = 1;
    let mut close = index + 1;

    while close < len && depth > 0 {
        if chars[close] == '{' {
            depth += 1;
        } else if chars[close] == '}' {
            depth -= 1;
        }
        if depth > 0 {
            close += 1;
        }
    }

    if depth == 0 {
        let inner: String = chars[index + 1..close].iter().collect();
        let is_numeric_range = if let Some((start_str, end_str)) = inner.split_once("..") {
            start_str.parse::<i64>().is_ok() && end_str.parse::<i64>().is_ok()
        } else {
            false
        };

        if is_numeric_range {
            let (start_str, end_str) = inner.split_once("..").unwrap();
            append_numeric_range(start_str, end_str, out);
        } else if inner.contains(',') {
            append_comma_choices(&inner, out)?;
        } else {
            out.push_str(r"\{");
            let inner_chars: Vec<char> = inner.chars().collect();
            translate_glob_tokens(&inner_chars, out)?;
            out.push_str(r"\}");
        }
        Ok(close + 1)
    } else {
        out.push_str(r"\{");
        Ok(index + 1)
    }
}

fn append_numeric_range(start_str: &str, end_str: &str, out: &mut String) {
    if let (Ok(start), Ok(end)) = (start_str.parse::<i64>(), end_str.parse::<i64>()) {
        let min = start.min(end);
        let max = start.max(end);
        if max - min <= 50 {
            out.push_str("(?:");
            for (idx, n) in (min..=max).enumerate() {
                if idx > 0 {
                    out.push('|');
                }
                out.push_str(&n.to_string());
            }
            out.push(')');
            return;
        }
    }
    out.push_str(r"\d+");
}

fn append_comma_choices(inner: &str, out: &mut String) -> Result<(), String> {
    out.push_str("(?:");
    let choices = split_nested_choices(inner);
    for (idx, choice) in choices.iter().enumerate() {
        if idx > 0 {
            out.push('|');
        }
        let choice_chars: Vec<char> = choice.chars().collect();
        translate_glob_tokens(&choice_chars, out)?;
    }
    out.push(')');
    Ok(())
}

fn split_nested_choices(inner: &str) -> Vec<String> {
    let mut choices = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for ch in inner.chars() {
        if ch == '{' {
            depth += 1;
            current.push(ch);
        } else if ch == '}' {
            depth -= 1;
            current.push(ch);
        } else if ch == ',' && depth == 0 {
            choices.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    choices.push(current.trim().to_string());
    choices
}

#[cfg(test)]
mod tests {
    use super::compile_glob;

    #[test]
    fn test_glob_matches_extension_choice() {
        let regex = compile_glob("*.{kt,kts}").unwrap();
        assert!(regex.is_match("Main.kt"));
        assert!(regex.is_match("build.gradle.kts"));
        assert!(regex.is_match("src/main/kotlin/App.kt"));
        assert!(!regex.is_match("Main.java"));
    }

    #[test]
    fn test_glob_matches_directory_anchored_pattern() {
        let regex = compile_glob("src/**.kt").unwrap();
        assert!(regex.is_match("src/Main.kt"));
        assert!(regex.is_match("src/nested/Deep.kt"));
        assert!(!regex.is_match("other/src/Main.kt"));
        assert!(!regex.is_match("Main.kt"));
    }

    #[test]
    fn test_glob_matches_character_class() {
        let regex = compile_glob("[a-z]*.kt").unwrap();
        assert!(regex.is_match("foo.kt"));
        assert!(!regex.is_match("Foo.kt"));
    }

    #[test]
    fn test_glob_negated_character_class_does_not_match_slash() {
        let regex = compile_glob("foo/[!a]bar.kt").unwrap();
        assert!(regex.is_match("foo/bbar.kt"));
        assert!(!regex.is_match("foo/abar.kt"));
        assert!(!regex.is_match("foo//bar.kt"));
    }

    #[test]
    fn test_glob_directory_wildcard_matches_zero_or_more_directories() {
        let regex = compile_glob("src/**/test.kt").unwrap();
        assert!(regex.is_match("src/test.kt"));
        assert!(regex.is_match("src/sub/test.kt"));
        assert!(regex.is_match("src/a/b/c/test.kt"));
        assert!(!regex.is_match("other/src/test.kt"));
    }

    #[test]
    fn test_glob_leading_wildcard_directories() {
        let regex = compile_glob("**/test.kt").unwrap();
        assert!(regex.is_match("test.kt"));
        assert!(regex.is_match("src/test.kt"));
        assert!(regex.is_match("a/b/c/test.kt"));
    }

    #[test]
    fn test_glob_numeric_range_and_non_numeric_braces() {
        let num_regex = compile_glob("file{1..3}.kt").unwrap();
        assert!(num_regex.is_match("file1.kt"));
        assert!(num_regex.is_match("file2.kt"));
        assert!(num_regex.is_match("file3.kt"));
        assert!(!num_regex.is_match("file4.kt"));

        let non_num_regex = compile_glob("prefix_{foo..bar}.kt").unwrap();
        assert!(non_num_regex.is_match("prefix_{foo..bar}.kt"));
        assert!(!non_num_regex.is_match("prefix_123.kt"));
    }

    #[test]
    fn test_glob_single_braced_element_matches_literal_braces() {
        let regex = compile_glob("prefix_{foo}.kt").unwrap();
        assert!(regex.is_match("prefix_{foo}.kt"));
        assert!(!regex.is_match("prefix_foo.kt"));
    }
}

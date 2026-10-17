use std::fs;
use std::path::{Path, PathBuf};

#[path = "build_source_masker.rs"]
mod build_source_masker;
use build_source_masker::SourceMasker;

pub struct LintRules {
    pub max_function_lines: usize,
    pub max_file_lines_warn: usize,
    pub max_file_lines_error: usize,
    pub max_indent_spaces: usize,
}

#[derive(Debug)]
pub struct Violation {
    pub file: String,
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
    pub suggestion: &'static str,
    pub is_error: bool,
}

impl Violation {
    fn new(
        file: &str,
        line: usize,
        rule: &'static str,
        message: String,
        suggestion: &'static str,
        is_error: bool,
    ) -> Self {
        Self {
            file: file.to_string(),
            line,
            rule,
            message,
            suggestion,
            is_error,
        }
    }
}

#[derive(Debug)]
struct FunctionFrame {
    name: String,
    start_line: usize,
    body_started: bool,
    brace_depth: usize,
    paren_depth: usize,
    bracket_depth: usize,
}

impl FunctionFrame {
    fn new(name: String, start_line: usize) -> Self {
        Self {
            name,
            start_line,
            body_started: false,
            brace_depth: 0,
            paren_depth: 0,
            bracket_depth: 0,
        }
    }
}

fn is_valid_fn_modifier_prefix(prefix: &str) -> bool {
    const MODS: &[&str] = &[
        "pub", "crate", "super", "self", "in", "async", "const", "unsafe", "extern", "default",
        "C", "system",
    ];
    prefix.split_whitespace().all(|token| {
        let clean = token.trim_matches(|c: char| c == '(' || c == ')' || c == '"' || c == ',');
        clean.is_empty() || MODS.contains(&clean)
    })
}

fn extract_fn_name(after_fn: &str) -> Option<String> {
    if after_fn.is_empty() || after_fn.starts_with('(') || after_fn.starts_with('<') {
        return None;
    }
    let (mut name, rest) = if let Some(stripped) = after_fn.strip_prefix("r#") {
        (String::from("r#"), stripped)
    } else {
        (String::new(), after_fn)
    };
    let first = rest.chars().next()?;
    if !first.is_ascii_alphabetic() && first != '_' {
        return None;
    }
    for c in rest.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            name.push(c);
        } else {
            break;
        }
    }
    Some(name)
}

fn extract_function_declaration(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut search_start = 0;
    while search_start < len {
        let fn_idx = search_start + line[search_start..].find("fn")?;
        let valid_left = fn_idx == 0
            || bytes[fn_idx - 1].is_ascii_whitespace()
            || bytes[fn_idx - 1] == b'{'
            || bytes[fn_idx - 1] == b';';
        let valid_right = fn_idx + 2 < len && bytes[fn_idx + 2].is_ascii_whitespace();
        if valid_left && valid_right {
            let start = line[..fn_idx].rfind(['{', '}', ';']).map_or(0, |i| i + 1);
            if is_valid_fn_modifier_prefix(&line[start..fn_idx])
                && let Some(name) = extract_fn_name(line[fn_idx + 2..].trim_start())
            {
                return Some(name);
            }
        }
        search_start = fn_idx + 2;
    }
    None
}

fn check_indentation(
    rules: &LintRules,
    line: &str,
    line_num: usize,
    display_path: &str,
    state: &mut Option<(usize, usize, usize)>,
    violations: &mut Vec<Violation>,
) {
    let trimmed = line.trim();
    if !trimmed.is_empty()
        && !trimmed.starts_with("//")
        && !trimmed.starts_with("/*")
        && !trimmed.starts_with('*')
    {
        let spaces: usize = line
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .map(|c| if c == '\t' { 4 } else { 1 })
            .sum();
        if spaces > rules.max_indent_spaces {
            match state {
                Some((_start, end, max_sp)) => {
                    *end = line_num;
                    *max_sp = (*max_sp).max(spaces);
                }
                None => *state = Some((line_num, line_num, spaces)),
            }
            return;
        }
    }
    if let Some((start, end, max_sp)) = state.take() {
        record_indent_violation(rules, display_path, start, end, max_sp, violations);
    }
}

fn update_frame_char(frame: &mut FunctionFrame, ch: char) {
    match ch {
        '{' if !frame.body_started => {
            frame.body_started = true;
            frame.brace_depth = 1;
        }
        '{' => frame.brace_depth += 1,
        '}' if frame.body_started && frame.brace_depth > 0 => frame.brace_depth -= 1,
        '(' if !frame.body_started => frame.paren_depth += 1,
        ')' if !frame.body_started && frame.paren_depth > 0 => frame.paren_depth -= 1,
        '[' if !frame.body_started => frame.bracket_depth += 1,
        ']' if !frame.body_started && frame.bracket_depth > 0 => frame.bracket_depth -= 1,
        ';' if !frame.body_started && frame.paren_depth == 0 && frame.bracket_depth == 0 => {
            frame.brace_depth = usize::MAX
        }
        _ => {}
    }
}

fn check_completed_functions(
    rules: &LintRules,
    stack: &mut Vec<FunctionFrame>,
    line_num: usize,
    display_path: &str,
    violations: &mut Vec<Violation>,
) {
    let mut i = 0;
    while i < stack.len() {
        if stack[i].brace_depth == usize::MAX {
            stack.remove(i);
        } else if stack[i].body_started && stack[i].brace_depth == 0 {
            let func = stack.remove(i);
            let count = line_num - func.start_line + 1;
            if count > rules.max_function_lines {
                let msg = format!(
                    "Function '{}' has {} lines (max {}).",
                    func.name, count, rules.max_function_lines
                );
                violations.push(Violation::new(
                    display_path,
                    func.start_line,
                    "max-function-lines",
                    msg,
                    "Split into smaller helper functions or extract sub-routines.",
                    true,
                ));
            }
        } else {
            i += 1;
        }
    }
}

fn record_indent_violation(
    rules: &LintRules,
    file: &str,
    start: usize,
    end: usize,
    spaces: usize,
    violations: &mut Vec<Violation>,
) {
    let loc = if start == end {
        format!("at line {}", start)
    } else {
        format!("at lines {}-{}", start, end)
    };
    let msg = format!(
        "Indentation of {} spaces exceeds max ({}) {}.",
        spaces, rules.max_indent_spaces, loc
    );
    violations.push(Violation::new(
        file,
        start,
        "max-indent-spaces",
        msg,
        "Reduce nesting using early returns, guard clauses, or helper functions.",
        true,
    ));
}

pub fn lint_file(
    rules: &LintRules,
    file_path: &Path,
    root: &Path,
    violations: &mut Vec<Violation>,
) {
    let Ok(content) = fs::read_to_string(file_path) else {
        return;
    };
    let display = file_path
        .strip_prefix(root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .replace('\\', "/");
    let lines: Vec<&str> = content.lines().collect();
    let line_count = lines.len();
    if line_count > rules.max_file_lines_warn {
        let msg = format!(
            "File has {} lines (warn: {}, max {}).",
            lines.len(),
            rules.max_file_lines_warn,
            rules.max_file_lines_error
        );
        violations.push(Violation::new(
            &display,
            1,
            "max-file-lines",
            msg,
            "Refactor or extract to new modules unless you are 100% sure this file will never be touched.",
            false,
        ));
    } else if line_count > rules.max_file_lines_error {
        let msg = format!(
            "File has {} lines (max {}).",
            lines.len(),
            rules.max_file_lines_error
        );
        violations.push(Violation::new(
            &display,
            1,
            "max-file-lines",
            msg,
            "New files are free",
            true,
        ));
    }
    let mut masker = SourceMasker::default();
    let mut stack: Vec<FunctionFrame> = Vec::new();
    let mut indent_state = None;

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        check_indentation(
            rules,
            line,
            line_num,
            &display,
            &mut indent_state,
            violations,
        );
        let masked = masker.mask_line(line);
        if let Some(name) = extract_function_declaration(&masked) {
            stack.push(FunctionFrame::new(name, line_num));
        }
        for ch in masked.chars() {
            for frame in &mut stack {
                update_frame_char(frame, ch);
            }
        }
        check_completed_functions(rules, &mut stack, line_num, &display, violations);
    }
    if let Some((start, end, sp)) = indent_state {
        record_indent_violation(rules, &display, start, end, sp, violations);
    }
}

pub fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name != "target" && !name.starts_with('.') {
                    collect_rs_files(&path, files);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }
}

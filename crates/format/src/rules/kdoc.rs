use smol_str::SmolStr;

use super::kdoc_words::{reflow_words, split_kdoc_words};
use crate::doc::Doc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KDocKind {
    Preformatted,
    DocTag,
    ListItem,
    Text,
}

pub struct KDocParagraph {
    pub kind: KDocKind,
    pub lines: Vec<String>,
    pub combined_text: String,
    pub is_priority: bool,
    pub had_blank_line_before: bool,
}

fn is_kdoc_tag(s: &str) -> bool {
    let Some(rest) = s.strip_prefix('@') else {
        return false;
    };
    let tag = rest.split_whitespace().next().unwrap_or(rest);
    let tag_name = tag.split_once('[').map_or(tag, |(b, _)| b);
    !tag_name.is_empty()
        && tag_name
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '@')
}

fn is_list_item(s: &str) -> bool {
    if s.starts_with("- ") || s.starts_with("* ") || s.starts_with("+ ") || s.starts_with("<li>") {
        return true;
    }
    if let Some(first) = s.chars().next()
        && first.is_ascii_digit()
    {
        let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
        let rest = &s[end..];
        return rest.starts_with(". ") || rest.starts_with(") ");
    }
    false
}

fn doc_tag_rank(tag_line: &str, is_priority: bool) -> i32 {
    if is_priority {
        return -1;
    }
    let tag = tag_line.split_whitespace().next().unwrap_or(tag_line);
    let name = tag.split_once('[').map_or(tag, |(b, _)| b);
    match name {
        "@param" | "@property" => 0,
        "@return" => 1,
        "@constructor" => 2,
        "@receiver" => 3,
        "@throws" => 4,
        "@exception" => 5,
        "@sample" => 6,
        "@see" => 7,
        "@author" => 8,
        "@since" => 9,
        "@suppress" => 10,
        "@deprecated" => 11,
        _ => 100,
    }
}

fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        let is_space = c == ' ';
        if !is_space || !prev_space {
            out.push(c);
        }
        prev_space = is_space;
    }
    out
}

#[derive(Default)]
struct KDocParser {
    paragraphs: Vec<KDocParagraph>,
    current_lines: Vec<String>,
    current_kind: Option<KDocKind>,
    current_priority: bool,
    had_blank_line: bool,
    in_code_block: bool,
    seen_first_doc_tag: bool,
}

impl KDocParser {
    fn flush(&mut self) {
        if self.current_lines.is_empty() {
            return;
        }
        let kind = self.current_kind.unwrap_or(KDocKind::Text);
        let combined_text = if kind == KDocKind::Preformatted {
            while self
                .current_lines
                .last()
                .is_some_and(|l| l.trim().is_empty())
            {
                self.current_lines.pop();
            }
            String::new()
        } else {
            self.current_lines.join(" ")
        };
        self.paragraphs.push(KDocParagraph {
            kind,
            lines: std::mem::take(&mut self.current_lines),
            combined_text,
            is_priority: self.current_priority,
            had_blank_line_before: self.had_blank_line,
        });
        self.current_kind = None;
        self.current_priority = false;
        self.had_blank_line = false;
    }

    fn process_line(&mut self, content: &str) {
        if self.in_code_block {
            self.current_lines.push(content.trim_end().to_string());
            if content.trim_start().starts_with("```") || content.contains("</pre>") {
                self.flush();
                self.in_code_block = false;
            }
            return;
        }
        let trimmed = content.trim();
        if trimmed.is_empty() {
            self.flush();
            self.had_blank_line = true;
            return;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("<pre>") {
            self.flush();
            self.in_code_block = true;
            self.current_kind = Some(KDocKind::Preformatted);
            self.current_lines.push(content.trim_end().to_string());
            return;
        }
        if is_kdoc_tag(trimmed) {
            self.flush();
            self.current_kind = Some(KDocKind::DocTag);
            self.current_priority = !self.seen_first_doc_tag && trimmed.starts_with("@sample");
            self.seen_first_doc_tag = true;
            self.current_lines.push(collapse_spaces(trimmed));
            return;
        }
        if is_list_item(trimmed) {
            self.flush();
            self.current_kind = Some(KDocKind::ListItem);
            self.current_lines.push(collapse_spaces(trimmed));
            return;
        }
        if self.current_kind.is_none() {
            self.current_kind = Some(KDocKind::Text);
        }
        self.current_lines.push(collapse_spaces(trimmed));
    }
}

fn parse_kdoc(comment: &str) -> Vec<KDocParagraph> {
    if comment.len() < 5 {
        return Vec::new();
    }
    let inner = &comment[3..comment.len() - 2];
    let is_multiline = inner.contains('\n');
    let mut parser = KDocParser::default();
    for (idx, raw_line) in inner.lines().enumerate() {
        let trimmed = raw_line.trim_start();
        let content = if (is_multiline && idx > 0 || trimmed.starts_with('*'))
            && !trimmed.starts_with("**")
            && let Some(stripped) = trimmed.strip_prefix('*')
        {
            stripped.strip_prefix(' ').unwrap_or(stripped)
        } else {
            trimmed
        };
        parser.process_line(content);
    }
    parser.flush();
    parser.paragraphs
}

fn emit_paragraph_doc<'a>(paragraph: &KDocParagraph, max_width: usize, docs: &mut Vec<Doc<'a>>) {
    if paragraph.kind == KDocKind::Preformatted {
        for line in &paragraph.lines {
            docs.push(Doc::HardLine);
            if line.is_empty() {
                docs.push(Doc::text(" *"));
            } else {
                docs.push(Doc::text(" * "));
                docs.push(Doc::smol(SmolStr::new(line)));
            }
        }
        return;
    }
    let is_hanging = matches!(paragraph.kind, KDocKind::DocTag | KDocKind::ListItem);
    let line1_max = max_width.saturating_sub(3);
    let cont_max = if is_hanging {
        max_width.saturating_sub(5)
    } else {
        line1_max
    };
    let words = split_kdoc_words(&paragraph.combined_text, is_hanging);
    for (i, line) in reflow_words(&words, line1_max, cont_max)
        .into_iter()
        .enumerate()
    {
        docs.push(Doc::HardLine);
        if i > 0 && is_hanging {
            docs.push(Doc::text(" *   "));
        } else {
            docs.push(Doc::text(" * "));
        }
        docs.push(Doc::smol(SmolStr::new(line)));
    }
}

fn stabilize_kdoc_paragraphs(
    paragraphs: Vec<KDocParagraph>,
    max_width: usize,
) -> Vec<KDocParagraph> {
    let mut has_wrapped_tag = false;
    let mut rebuilt = String::from("/**\n");
    for p in &paragraphs {
        if p.had_blank_line_before {
            rebuilt.push_str(" *\n");
        }
        if p.kind == KDocKind::Preformatted {
            for l in &p.lines {
                rebuilt.push_str(" * ");
                rebuilt.push_str(l);
                rebuilt.push('\n');
            }
            continue;
        }
        let is_hanging = matches!(p.kind, KDocKind::DocTag | KDocKind::ListItem);
        let line1_max = max_width.saturating_sub(3);
        let cont_max = if is_hanging {
            max_width.saturating_sub(5)
        } else {
            line1_max
        };
        let words = split_kdoc_words(&p.combined_text, is_hanging);
        for (i, line) in reflow_words(&words, line1_max, cont_max)
            .into_iter()
            .enumerate()
        {
            if i > 0 && is_kdoc_tag(line.trim_start()) {
                has_wrapped_tag = true;
            }
            rebuilt.push_str(" * ");
            rebuilt.push_str(&line);
            rebuilt.push('\n');
        }
    }
    rebuilt.push_str(" */");
    if has_wrapped_tag {
        parse_kdoc(&rebuilt)
    } else {
        paragraphs
    }
}

pub fn format_kdoc(comment: &str, max_width: usize) -> Doc<'static> {
    let comment_trimmed = comment.trim();
    if !comment_trimmed.starts_with("/**") || !comment_trimmed.ends_with("*/") {
        return Doc::smol(SmolStr::new(comment_trimmed));
    }
    if comment_trimmed.len() <= 4 {
        return Doc::smol(SmolStr::new(comment_trimmed));
    }
    let paragraphs = parse_kdoc(comment_trimmed);
    if paragraphs.is_empty() {
        return Doc::text("/** */");
    }
    if paragraphs.len() == 1
        && paragraphs[0].kind == KDocKind::Text
        && paragraphs[0].combined_text.len() + 7 <= max_width
    {
        return Doc::smol(common::concat_smol_str!(
            "/** ",
            paragraphs[0].combined_text.as_str(),
            " */"
        ));
    }
    let mut paragraphs = stabilize_kdoc_paragraphs(paragraphs, max_width);

    let mut i = 0;
    while i < paragraphs.len() {
        if paragraphs[i].kind == KDocKind::DocTag {
            let start = i;
            while i < paragraphs.len() && paragraphs[i].kind == KDocKind::DocTag {
                i += 1;
            }
            paragraphs[start..i].sort_by_key(|p| doc_tag_rank(&p.combined_text, p.is_priority));
        } else {
            i += 1;
        }
    }

    let mut docs = Vec::new();
    docs.push(Doc::text("/**"));
    let mut prev_paragraph: Option<&KDocParagraph> = None;

    for paragraph in &paragraphs {
        if let Some(prev) = prev_paragraph {
            let prev_ends_with_colon = prev.combined_text.trim_end().ends_with(':')
                || prev
                    .lines
                    .last()
                    .is_some_and(|l| l.trim_end().ends_with(':'));
            let needs_blank = match paragraph.kind {
                KDocKind::DocTag => prev.kind != KDocKind::DocTag,
                KDocKind::ListItem => paragraph.had_blank_line_before,
                KDocKind::Preformatted => !prev_ends_with_colon,
                KDocKind::Text => paragraph.had_blank_line_before || prev.kind != KDocKind::Text,
            };
            if needs_blank {
                docs.push(Doc::HardLine);
                docs.push(Doc::text(" *"));
            }
        }
        emit_paragraph_doc(paragraph, max_width, &mut docs);
        prev_paragraph = Some(paragraph);
    }

    docs.push(Doc::HardLine);
    docs.push(Doc::text(" */"));
    Doc::Concat(docs)
}

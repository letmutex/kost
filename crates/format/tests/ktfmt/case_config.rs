use format::{FileType, FormattingOptions, TrailingCommaStrategy};

/// Configuration parsed from top-of-file directives in test fixtures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseConfig {
    pub options: FormattingOptions,
    pub file_type: FileType,
    pub check_idempotency: bool,
}

impl CaseConfig {
    /// Parses case configuration directives from test fixture header comments.
    pub fn parse(source: &str, base_options: &FormattingOptions) -> Result<Self, String> {
        let (has_shebang, remaining_lines) = Self::strip_shebang(source);
        let mut options = base_options.clone();
        let mut file_type = if has_shebang {
            FileType::Script
        } else {
            FileType::Regular
        };
        let mut check_idempotency = true;
        let mut directives_ended = false;
        let line_offset = if has_shebang { 2 } else { 1 };

        for (idx, line) in remaining_lines.iter().enumerate() {
            let trimmed = line.trim();
            if !trimmed.starts_with("//") {
                directives_ended = true;
                continue;
            }

            let comment_body = trimmed.strip_prefix("//").unwrap_or("").trim();
            let mut parts = comment_body.split_whitespace();
            let Some(directive) = parts.next() else {
                continue;
            };
            let value = parts.next();

            let is_directive_token = directive
                .chars()
                .all(|c| c.is_ascii_uppercase() || c == '_');
            if !is_directive_token {
                directives_ended = true;
                continue;
            }

            if directives_ended {
                let actual_line = idx + line_offset;
                return Err(format!(
                    "Line {actual_line}: directive '{directive}' must be listed at top"
                ));
            }

            Self::apply_directive(
                directive,
                value,
                &mut options,
                &mut file_type,
                &mut check_idempotency,
            )?;
        }

        Ok(Self {
            options,
            file_type,
            check_idempotency,
        })
    }

    fn strip_shebang(source: &str) -> (bool, Vec<&str>) {
        let mut lines = source.lines();
        let first_line = lines.next().unwrap_or("");
        if first_line.starts_with("#!") {
            (true, lines.collect())
        } else {
            let mut all = Vec::with_capacity(source.lines().count());
            all.push(first_line);
            all.extend(lines);
            (false, all)
        }
    }

    fn apply_directive(
        directive: &str,
        value: Option<&str>,
        options: &mut FormattingOptions,
        file_type: &mut FileType,
        check_idempotency: &mut bool,
    ) -> Result<(), String> {
        match directive {
            "FILE_TYPE" => Self::apply_file_type(value, file_type),
            "MAX_WIDTH" => Self::apply_max_width(value, options),
            "BLOCK_INDENT" => Self::apply_block_indent(value, options),
            "CONTINUATION_INDENT" => Self::apply_continuation_indent(value, options),
            "TRAILING_COMMA_STRATEGY" => Self::apply_trailing_comma(value, options),
            "REMOVE_UNUSED_IMPORTS" => Self::apply_remove_unused_imports(value, options),
            "PRESERVE_LAMBDA_BREAKS" => Self::apply_preserve_lambda_breaks(value, options),
            "CHECK_IDEMPOTENCY" => Self::apply_check_idempotency(value, check_idempotency),
            "PRINT_OPTS_AFTER_FORMATTING" => Ok(()),
            unknown => Err(format!("Unknown directive '{unknown}'")),
        }
    }

    fn apply_file_type(value: Option<&str>, file_type: &mut FileType) -> Result<(), String> {
        *file_type = match value {
            Some("REGULAR") => FileType::Regular,
            Some("SCRIPT") => FileType::Script,
            Some(other) => return Err(format!("Unsupported FILE_TYPE: {other}")),
            None => return Err("FILE_TYPE requires an argument".to_string()),
        };
        Ok(())
    }

    fn apply_max_width(value: Option<&str>, options: &mut FormattingOptions) -> Result<(), String> {
        let text = value.ok_or_else(|| "MAX_WIDTH requires an integer".to_string())?;
        options.max_width = text
            .parse()
            .map_err(|e| format!("Invalid MAX_WIDTH: {e}"))?;
        Ok(())
    }

    fn apply_block_indent(
        value: Option<&str>,
        options: &mut FormattingOptions,
    ) -> Result<(), String> {
        let text = value.ok_or_else(|| "BLOCK_INDENT requires an integer".to_string())?;
        options.block_indent = text
            .parse()
            .map_err(|e| format!("Invalid BLOCK_INDENT: {e}"))?;
        Ok(())
    }

    fn apply_continuation_indent(
        value: Option<&str>,
        options: &mut FormattingOptions,
    ) -> Result<(), String> {
        let text = value.ok_or_else(|| "CONTINUATION_INDENT requires an integer".to_string())?;
        options.continuation_indent = text
            .parse()
            .map_err(|e| format!("Invalid CONTINUATION_INDENT: {e}"))?;
        Ok(())
    }

    fn apply_trailing_comma(
        value: Option<&str>,
        options: &mut FormattingOptions,
    ) -> Result<(), String> {
        options.trailing_comma_strategy = match value {
            Some("NONE") => TrailingCommaStrategy::None,
            Some("ONLY_ADD") => TrailingCommaStrategy::OnlyAdd,
            Some("COMPLETE") => TrailingCommaStrategy::Complete,
            Some(other) => return Err(format!("Invalid TRAILING_COMMA_STRATEGY: {other}")),
            None => return Err("TRAILING_COMMA_STRATEGY requires an argument".to_string()),
        };
        Ok(())
    }

    fn apply_remove_unused_imports(
        value: Option<&str>,
        options: &mut FormattingOptions,
    ) -> Result<(), String> {
        let text = value.ok_or_else(|| "REMOVE_UNUSED_IMPORTS requires a boolean".to_string())?;
        options.remove_unused_imports =
            text.parse().map_err(|e| format!("Invalid boolean: {e}"))?;
        Ok(())
    }

    fn apply_preserve_lambda_breaks(
        value: Option<&str>,
        options: &mut FormattingOptions,
    ) -> Result<(), String> {
        let text = value.ok_or_else(|| "PRESERVE_LAMBDA_BREAKS requires a boolean".to_string())?;
        options.preserve_lambda_breaks =
            text.parse().map_err(|e| format!("Invalid boolean: {e}"))?;
        Ok(())
    }

    fn apply_check_idempotency(
        value: Option<&str>,
        check_idempotency: &mut bool,
    ) -> Result<(), String> {
        let text = value.ok_or_else(|| "CHECK_IDEMPOTENCY requires a boolean".to_string())?;
        *check_idempotency = text.parse().map_err(|e| format!("Invalid boolean: {e}"))?;
        Ok(())
    }
}

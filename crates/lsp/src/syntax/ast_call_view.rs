use tree_sitter::Node;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCallArg {
    pub name: Option<String>,
    pub expr: String,
}

/// Find the top-level '=' in an expression or parameter (ignoring nested structures, strings, and compound operators)
pub fn find_top_level_eq(s: &str) -> Option<usize> {
    let mut depth = 0;
    let mut in_string = false;
    let bytes = s.as_bytes();
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if in_string {
            if c == '\\' {
                chars.next();
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '<' | '(' | '{' | '[' => depth += 1,
            ')' | '}' | ']' if depth > 0 => depth -= 1,
            '>' if depth > 0 && (i == 0 || bytes[i - 1] != b'-') => depth -= 1,
            '=' if depth == 0 => {
                let prev = if i > 0 {
                    bytes.get(i - 1).copied()
                } else {
                    None
                };
                let next = bytes.get(i + 1).copied();
                let is_compound = matches!(prev, Some(b'=' | b'!' | b'<' | b'>'))
                    || matches!(next, Some(b'=' | b'>'));
                if !is_compound {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse a call argument into optional name and expression
pub fn parse_call_arg(raw: &str) -> ParsedCallArg {
    let trimmed = raw.trim();
    if let Some(eq_idx) = find_top_level_eq(trimmed)
        && eq_idx > 0
    {
        let name_cand = trimmed[..eq_idx].trim();
        let name_clean = name_cand.trim_matches('`');
        let first_valid = name_clean
            .chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$');
        if first_valid
            && name_clean
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '$')
        {
            return ParsedCallArg {
                name: Some(name_clean.to_string()),
                expr: trimmed[eq_idx + 1..].trim().to_string(),
            };
        }
    }

    ParsedCallArg {
        name: None,
        expr: trimmed.to_string(),
    }
}

/// Structured view of a Kotlin call expression AST node without string slicing
#[derive(Debug, Clone)]
pub struct AstCallView<'a> {
    pub call_node: Node<'a>,
    pub callee_node: Option<Node<'a>>,
    pub call_suffix_node: Option<Node<'a>>,
    pub type_arguments_node: Option<Node<'a>>,
    pub type_arguments: Vec<Node<'a>>,
    pub value_arguments_node: Option<Node<'a>>,
    pub value_arguments: Vec<Node<'a>>,
    pub trailing_lambda: Option<Node<'a>>,
}

impl<'a> AstCallView<'a> {
    pub fn from_node(node: Node<'a>) -> Option<Self> {
        let kind = node.kind();
        if kind != "call_expression" && kind != "constructor_invocation" {
            return None;
        }

        let mut callee_node = None;
        let mut call_suffix_node = None;
        let mut type_arguments_node = None;
        let mut type_arguments = Vec::new();
        let mut value_arguments_node = None;
        let mut value_arguments = Vec::new();
        let mut trailing_lambda = None;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "call_suffix" => {
                    call_suffix_node = Some(child);
                    Self::unpack_suffix(
                        child,
                        &mut type_arguments_node,
                        &mut type_arguments,
                        &mut value_arguments_node,
                        &mut value_arguments,
                        &mut trailing_lambda,
                    );
                }
                "type_arguments" => {
                    type_arguments_node = Some(child);
                    Self::collect_type_args(child, &mut type_arguments);
                }
                "value_arguments" => {
                    value_arguments_node = Some(child);
                    Self::collect_value_args(child, &mut value_arguments);
                }
                "annotated_lambda" | "lambda_literal" => {
                    trailing_lambda = Some(child);
                }
                _ => {
                    if callee_node.is_none() && child.is_named() {
                        callee_node = Some(child);
                    }
                }
            }
        }

        Self::unwrap_trailing_lambda_inner_call(
            &mut callee_node,
            &mut call_suffix_node,
            &mut type_arguments_node,
            &mut type_arguments,
            &mut value_arguments_node,
            &mut value_arguments,
            trailing_lambda,
        );

        Some(Self {
            call_node: node,
            callee_node,
            call_suffix_node,
            type_arguments_node,
            type_arguments,
            value_arguments_node,
            value_arguments,
            trailing_lambda,
        })
    }

    fn unwrap_trailing_lambda_inner_call(
        callee_node: &mut Option<Node<'a>>,
        call_suffix_node: &mut Option<Node<'a>>,
        type_arguments_node: &mut Option<Node<'a>>,
        type_arguments: &mut Vec<Node<'a>>,
        value_arguments_node: &mut Option<Node<'a>>,
        value_arguments: &mut Vec<Node<'a>>,
        trailing_lambda: Option<Node<'a>>,
    ) {
        if trailing_lambda.is_some()
            && value_arguments.is_empty()
            && let Some(inner) = *callee_node
            && inner.kind() == "call_expression"
            && let Some(inner_view) = Self::from_node(inner)
            && inner_view.trailing_lambda.is_none()
            && let Some(inner_callee) = inner_view.callee_node
        {
            *callee_node = Some(inner_callee);
            if type_arguments.is_empty() {
                *type_arguments_node = inner_view.type_arguments_node;
                *type_arguments = inner_view.type_arguments;
            }
            *value_arguments_node = inner_view.value_arguments_node;
            *value_arguments = inner_view.value_arguments;
            if call_suffix_node.is_none() {
                *call_suffix_node = inner_view.call_suffix_node;
            }
        }
    }

    fn unpack_suffix(
        suffix: Node<'a>,
        type_args_node: &mut Option<Node<'a>>,
        type_args: &mut Vec<Node<'a>>,
        val_args_node: &mut Option<Node<'a>>,
        val_args: &mut Vec<Node<'a>>,
        trailing_lambda: &mut Option<Node<'a>>,
    ) {
        let mut cursor = suffix.walk();
        for child in suffix.children(&mut cursor) {
            match child.kind() {
                "type_arguments" => {
                    *type_args_node = Some(child);
                    Self::collect_type_args(child, type_args);
                }
                "value_arguments" => {
                    *val_args_node = Some(child);
                    Self::collect_value_args(child, val_args);
                }
                "annotated_lambda" | "lambda_literal" => {
                    *trailing_lambda = Some(child);
                }
                _ => {}
            }
        }
    }

    fn collect_type_args(node: Node<'a>, out: &mut Vec<Node<'a>>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named()
                && !matches!(
                    child.kind(),
                    "comment" | "line_comment" | "multiline_comment"
                )
            {
                out.push(child);
            }
        }
    }

    fn collect_value_args(node: Node<'a>, out: &mut Vec<Node<'a>>) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "value_argument" {
                out.push(child);
            }
        }
    }

    /// Extract the expression node from a value_argument node
    pub fn argument_expression(val_arg: Node<'a>) -> Option<Node<'a>> {
        let mut cursor = val_arg.walk();
        let mut expr = None;
        let mut past_equals = false;
        for child in val_arg.children(&mut cursor) {
            if child.kind() == "=" {
                past_equals = true;
            } else if child.is_named() {
                if past_equals {
                    return Some(child);
                }
                expr = Some(child);
            }
        }
        expr
    }

    /// Extract argument name if the argument is a named argument (e.g. `foo = 42`)
    pub fn argument_name(val_arg: Node<'a>, source: &'a str) -> Option<&'a str> {
        let mut cursor = val_arg.walk();
        let mut has_equals = false;
        let mut first_ident = None;
        for child in val_arg.children(&mut cursor) {
            if child.kind() == "simple_identifier" && first_ident.is_none() && !has_equals {
                first_ident = Some(child);
            } else if child.kind() == "=" {
                has_equals = true;
                break;
            }
        }
        if has_equals {
            first_ident.and_then(|id| source.get(id.start_byte()..id.end_byte()).map(str::trim))
        } else {
            None
        }
    }

    /// Callee simple text
    pub fn callee_text(&self, source: &'a str) -> Option<&'a str> {
        let callee = self.callee_node?;
        source
            .get(callee.start_byte()..callee.end_byte())
            .map(str::trim)
    }

    /// Number of total arguments including trailing lambda
    pub fn total_argument_count(&self) -> usize {
        self.value_arguments.len() + if self.trailing_lambda.is_some() { 1 } else { 0 }
    }

    /// Unwrap an annotated_lambda or lambda_literal to its inner lambda_literal node
    pub fn unwrap_lambda(node: Node<'a>) -> Option<Node<'a>> {
        if node.kind() == "annotated_lambda" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "lambda_literal" {
                    return Some(child);
                }
            }
            None
        } else if node.kind() == "lambda_literal" {
            Some(node)
        } else {
            None
        }
    }
}

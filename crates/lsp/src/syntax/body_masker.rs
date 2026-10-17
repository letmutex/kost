/// Fast, zero-allocation-where-possible function body masker.
/// Replaces the contents of Kotlin function bodies `{ ... }` with spaces,
/// preserving all newlines, so that line numbers and byte offsets remain identical.
pub fn mask_function_bodies(source: &str) -> String {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut out = bytes.to_vec();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'/' && i + 1 < len {
            if bytes[i + 1] == b'/' {
                i = skip_line_comment(bytes, i + 2, len);
                continue;
            } else if bytes[i + 1] == b'*' {
                i = skip_block_comment(bytes, i + 2, len);
                continue;
            }
        }

        if bytes[i] == b'"' {
            i = skip_string(bytes, i, len);
            continue;
        }

        if bytes[i] == b'\'' {
            i = skip_char(bytes, i, len);
            continue;
        }

        if is_fun_keyword(bytes, i, len) {
            i = handle_fun_declaration(bytes, &mut out, i + 3, len);
            continue;
        }

        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| source.to_string())
}

fn is_fun_keyword(bytes: &[u8], i: usize, len: usize) -> bool {
    if i + 3 > len || &bytes[i..i + 3] != b"fun" {
        return false;
    }
    let prev_ok = i == 0 || !bytes[i - 1].is_ascii_alphanumeric() && bytes[i - 1] != b'_';
    let next_ok = i + 3 == len || bytes[i + 3].is_ascii_whitespace();
    prev_ok && next_ok
}

fn handle_fun_declaration(bytes: &[u8], out: &mut [u8], mut i: usize, len: usize) -> usize {
    i = skip_whitespace(bytes, i, len);
    if is_word_at(bytes, i, len, b"interface") {
        return i + 9;
    }

    let mut paren_depth = 0;
    while i < len {
        let b = bytes[i];
        if b == b'/' && i + 1 < len {
            if bytes[i + 1] == b'/' {
                i = skip_line_comment(bytes, i + 2, len);
                continue;
            } else if bytes[i + 1] == b'*' {
                i = skip_block_comment(bytes, i + 2, len);
                continue;
            }
        }
        if b == b'"' {
            i = skip_string(bytes, i, len);
            continue;
        }
        if b == b'\'' {
            i = skip_char(bytes, i, len);
            continue;
        }

        if b == b'(' {
            paren_depth += 1;
        } else if b == b')' && paren_depth > 0 {
            paren_depth -= 1;
        } else if paren_depth == 0 {
            if b == b'=' || b == b';' {
                return i + 1;
            }
            if b == b'}' || is_declaration_keyword(bytes, i, len) {
                return i;
            }
            if b == b'{' {
                return mask_block_body(bytes, out, i + 1, len);
            }
        }
        i += 1;
    }
    i
}

fn mask_block_body(bytes: &[u8], out: &mut [u8], mut i: usize, len: usize) -> usize {
    let mut depth = 1;
    while i < len && depth > 0 {
        let b = bytes[i];
        if b == b'/' && i + 1 < len {
            if bytes[i + 1] == b'/' {
                let end = skip_line_comment(bytes, i + 2, len);
                mask_range(bytes, out, i, end);
                i = end;
                continue;
            } else if bytes[i + 1] == b'*' {
                let end = skip_block_comment(bytes, i + 2, len);
                mask_range(bytes, out, i, end);
                i = end;
                continue;
            }
        }
        if b == b'"' {
            let end = skip_string(bytes, i, len);
            mask_range(bytes, out, i, end);
            i = end;
            continue;
        }
        if b == b'\'' {
            let end = skip_char(bytes, i, len);
            mask_range(bytes, out, i, end);
            i = end;
            continue;
        }

        if b == b'{' {
            depth += 1;
            out[i] = b' ';
        } else if b == b'}' {
            depth -= 1;
            if depth > 0 {
                out[i] = b' ';
            }
        } else if b != b'\n' && b != b'\r' {
            out[i] = b' ';
        }
        i += 1;
    }
    i
}

#[inline]
fn mask_range(bytes: &[u8], out: &mut [u8], start: usize, end: usize) {
    for idx in start..end.min(bytes.len()) {
        if bytes[idx] != b'\n' && bytes[idx] != b'\r' {
            out[idx] = b' ';
        }
    }
}

fn skip_whitespace(bytes: &[u8], mut i: usize, len: usize) -> usize {
    while i < len && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn is_word_at(bytes: &[u8], i: usize, len: usize, word: &[u8]) -> bool {
    let wlen = word.len();
    if i + wlen > len || &bytes[i..i + wlen] != word {
        return false;
    }
    i + wlen == len || bytes[i + wlen].is_ascii_whitespace() || bytes[i + wlen] == b'{'
}

fn is_declaration_keyword(bytes: &[u8], i: usize, len: usize) -> bool {
    const KEYWORDS: &[&[u8]] = &[
        b"class",
        b"interface",
        b"object",
        b"fun",
        b"val",
        b"var",
        b"override",
        b"open",
        b"abstract",
        b"private",
        b"protected",
        b"public",
        b"internal",
    ];
    for kw in KEYWORDS {
        if is_word_at(bytes, i, len, kw) {
            let prev_ok = i == 0 || bytes[i - 1].is_ascii_whitespace() || bytes[i - 1] == b'}';
            if prev_ok {
                return true;
            }
        }
    }
    false
}

fn skip_line_comment(bytes: &[u8], mut i: usize, len: usize) -> usize {
    while i < len && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

fn skip_block_comment(bytes: &[u8], mut i: usize, len: usize) -> usize {
    let mut nest = 1;
    while i + 1 < len && nest > 0 {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            nest -= 1;
            i += 2;
        } else if bytes[i] == b'/' && bytes[i + 1] == b'*' {
            nest += 1;
            i += 2;
        } else {
            i += 1;
        }
    }
    i
}

fn skip_string(bytes: &[u8], mut i: usize, len: usize) -> usize {
    if i + 2 < len && bytes[i + 1] == b'"' && bytes[i + 2] == b'"' {
        i += 3;
        while i + 2 < len {
            if bytes[i] == b'"' && bytes[i + 1] == b'"' && bytes[i + 2] == b'"' {
                return i + 3;
            }
            i += 1;
        }
        return len;
    }
    i += 1;
    while i < len {
        if bytes[i] == b'\\' {
            i += 2;
        } else if bytes[i] == b'"' {
            return i + 1;
        } else {
            i += 1;
        }
    }
    i
}

fn skip_char(bytes: &[u8], mut i: usize, len: usize) -> usize {
    i += 1;
    while i < len {
        if bytes[i] == b'\\' {
            i += 2;
        } else if bytes[i] == b'\'' {
            return i + 1;
        } else if bytes[i] == b'\n' {
            return i;
        } else {
            i += 1;
        }
    }
    i
}

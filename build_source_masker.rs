#[derive(Default)]
pub struct SourceMasker {
    in_block_comment: usize,
    in_raw_string_hashes: Option<usize>,
}

impl SourceMasker {
    pub fn mask_line(&mut self, line: &str) -> String {
        let bytes = line.as_bytes();
        let len = bytes.len();
        let mut out = vec![b' '; len];
        let mut i = 0;
        while i < len {
            if self.in_block_comment > 0 {
                i = self.skip_block_comment(bytes, i, len);
            } else if let Some(hashes) = self.in_raw_string_hashes {
                i = self.skip_raw_string(bytes, i, len, hashes);
            } else if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'/' {
                break;
            } else if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                self.in_block_comment += 1;
                i += 2;
            } else if bytes[i] == b'r'
                && i + 1 < len
                && (bytes[i + 1] == b'#' || bytes[i + 1] == b'"')
            {
                i = self.start_raw_string(bytes, i, len);
            } else if bytes[i] == b'"' {
                i = skip_string_literal(bytes, i, len);
            } else if bytes[i] == b'\'' {
                i = skip_char_literal(bytes, i, len, &mut out);
            } else {
                out[i] = bytes[i];
                i += 1;
            }
        }
        String::from_utf8(out).unwrap_or_else(|_| line.to_string())
    }

    fn skip_block_comment(&mut self, bytes: &[u8], i: usize, len: usize) -> usize {
        if i + 1 < len && bytes[i] == b'*' && bytes[i + 1] == b'/' {
            self.in_block_comment -= 1;
            i + 2
        } else if i + 1 < len && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            self.in_block_comment += 1;
            i + 2
        } else {
            i + 1
        }
    }

    fn skip_raw_string(&mut self, bytes: &[u8], i: usize, len: usize, hashes: usize) -> usize {
        if bytes[i] == b'"'
            && i + 1 + hashes <= len
            && bytes[i + 1..i + 1 + hashes].iter().all(|&b| b == b'#')
        {
            self.in_raw_string_hashes = None;
            i + 1 + hashes
        } else {
            i + 1
        }
    }

    fn start_raw_string(&mut self, bytes: &[u8], mut i: usize, len: usize) -> usize {
        let mut h_count = 0;
        i += 1;
        while i < len && bytes[i] == b'#' {
            h_count += 1;
            i += 1;
        }
        if i < len && bytes[i] == b'"' {
            self.in_raw_string_hashes = Some(h_count);
            i + 1
        } else {
            i
        }
    }
}

fn skip_string_literal(bytes: &[u8], mut i: usize, len: usize) -> usize {
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

fn skip_char_literal(bytes: &[u8], i: usize, len: usize, out: &mut [u8]) -> usize {
    if i + 2 < len && bytes[i + 1] != b'\\' && bytes[i + 2] == b'\'' {
        i + 3
    } else if i + 3 < len && bytes[i + 1] == b'\\' && bytes[i + 3] == b'\'' {
        i + 4
    } else {
        out[i] = bytes[i];
        i + 1
    }
}

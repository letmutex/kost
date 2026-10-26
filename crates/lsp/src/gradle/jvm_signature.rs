#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JvmMethodSignature {
    pub type_params: Vec<String>,
    pub params: Vec<String>,
    pub return_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JvmClassSignature {
    pub type_params: Vec<String>,
    pub supertypes: Vec<String>,
}

pub fn parse_jvm_type_signature(sig: &str) -> Option<String> {
    let mut parser = JvmSigParser::new(sig);
    parser.parse_type()
}

pub fn parse_jvm_method_signature(sig: &str) -> Option<JvmMethodSignature> {
    let mut parser = JvmSigParser::new(sig);
    parser.parse_method()
}

pub fn parse_jvm_class_signature(sig: &str) -> Option<JvmClassSignature> {
    let mut parser = JvmSigParser::new(sig);
    parser.parse_class_signature()
}

pub fn jvm_method_sig_to_kotlin(
    name: &str,
    method_sig: &JvmMethodSignature,
    is_extension: bool,
) -> (String, Option<String>) {
    let tp_prefix = if method_sig.type_params.is_empty() {
        String::new()
    } else {
        common::concat_str!("<", method_sig.type_params.join(", "), "> ")
    };

    let ret_suffix = if method_sig.return_type == "Unit" {
        String::new()
    } else {
        common::concat_str!(": ", method_sig.return_type)
    };

    if is_extension && !method_sig.params.is_empty() {
        let receiver = &method_sig.params[0];
        let arg_params: Vec<String> = method_sig
            .params
            .iter()
            .skip(1)
            .enumerate()
            .map(|(i, p)| common::concat_str!("arg", (i + 1).to_string(), ": ", p))
            .collect();
        let sig = common::concat_str!(
            "fun ",
            tp_prefix,
            receiver,
            ".",
            name,
            "(",
            arg_params.join(", "),
            ")",
            ret_suffix
        );
        (sig, Some(receiver.clone()))
    } else {
        let arg_params: Vec<String> = method_sig
            .params
            .iter()
            .enumerate()
            .map(|(i, p)| common::concat_str!("arg", (i + 1).to_string(), ": ", p))
            .collect();
        let sig = common::concat_str!(
            "fun ",
            tp_prefix,
            name,
            "(",
            arg_params.join(", "),
            ")",
            ret_suffix
        );
        (sig, None)
    }
}

struct JvmSigParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JvmSigParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<u8> {
        if self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            self.pos += 1;
            Some(b)
        } else {
            None
        }
    }

    fn consume(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_type(&mut self) -> Option<String> {
        let b = self.next()?;
        match b {
            b'V' => Some("Unit".to_string()),
            b'Z' => Some("Boolean".to_string()),
            b'B' => Some("Byte".to_string()),
            b'C' => Some("Char".to_string()),
            b'S' => Some("Short".to_string()),
            b'I' => Some("Int".to_string()),
            b'J' => Some("Long".to_string()),
            b'F' => Some("Float".to_string()),
            b'D' => Some("Double".to_string()),
            b'[' => {
                let inner = self.parse_type()?;
                Some(common::concat_str!("Array<", inner, ">"))
            }
            b'T' => {
                let start = self.pos;
                while self.pos < self.bytes.len() && self.bytes[self.pos] != b';' {
                    self.pos += 1;
                }
                let ident = std::str::from_utf8(&self.bytes[start..self.pos]).ok()?;
                self.consume(b';');
                Some(ident.to_string())
            }
            b'L' => self.parse_class_type(),
            _ => None,
        }
    }

    fn parse_type_args(&mut self) -> Option<Vec<String>> {
        let mut args = Vec::new();
        while self.pos < self.bytes.len() && self.peek() != Some(b'>') {
            if self.consume(b'*') {
                args.push("*".to_string());
            } else if self.consume(b'+') {
                let arg = self.parse_type()?;
                args.push(common::concat_str!("out ", arg));
            } else if self.consume(b'-') {
                let arg = self.parse_type()?;
                args.push(common::concat_str!("in ", arg));
            } else {
                let arg = self.parse_type()?;
                args.push(arg);
            }
        }
        self.consume(b'>');
        Some(args)
    }

    fn parse_class_type(&mut self) -> Option<String> {
        let start = self.pos;
        while self.pos < self.bytes.len()
            && self.bytes[self.pos] != b'<'
            && self.bytes[self.pos] != b';'
            && self.bytes[self.pos] != b'.'
        {
            self.pos += 1;
        }
        let raw_name = std::str::from_utf8(&self.bytes[start..self.pos]).ok()?;
        let simple_name = raw_name.rsplit('/').next().unwrap_or(raw_name);
        let mut result = simple_name.to_string();

        if self.consume(b'<') {
            let args = self.parse_type_args()?;
            if !args.is_empty() {
                result = common::concat_str!(result, "<", args.join(", "), ">");
            }
        }

        while self.consume(b'.') {
            let inner_start = self.pos;
            while self.pos < self.bytes.len()
                && self.bytes[self.pos] != b'<'
                && self.bytes[self.pos] != b';'
                && self.bytes[self.pos] != b'.'
            {
                self.pos += 1;
            }
            let inner_name = std::str::from_utf8(&self.bytes[inner_start..self.pos]).ok()?;
            result.push('.');
            result.push_str(inner_name);
            if self.consume(b'<') {
                let args = self.parse_type_args()?;
                if !args.is_empty() {
                    result = common::concat_str!(result, "<", args.join(", "), ">");
                }
            }
        }

        self.consume(b';');
        Some(result)
    }

    fn parse_type_params(&mut self) -> Option<Vec<String>> {
        if !self.consume(b'<') {
            return Some(Vec::new());
        }
        let mut type_params = Vec::new();
        while self.pos < self.bytes.len() && self.peek() != Some(b'>') {
            let id_start = self.pos;
            while self.pos < self.bytes.len()
                && self.bytes[self.pos] != b':'
                && self.bytes[self.pos] != b'>'
            {
                self.pos += 1;
            }
            let name = std::str::from_utf8(&self.bytes[id_start..self.pos])
                .ok()?
                .to_string();
            if !self.consume(b':') {
                break;
            }

            let mut bounds = Vec::new();
            if self.peek() != Some(b':')
                && self.peek() != Some(b'>')
                && let Some(b) = self.parse_type()
                && b != "Object"
                && b != "Any"
            {
                bounds.push(b);
            }
            while self.consume(b':') {
                if let Some(b) = self.parse_type() {
                    bounds.push(b);
                }
            }
            if bounds.is_empty() {
                type_params.push(name);
            } else {
                type_params.push(common::concat_str!(name, " : ", bounds.join(" & ")));
            }
        }
        self.consume(b'>');
        Some(type_params)
    }

    fn parse_method(&mut self) -> Option<JvmMethodSignature> {
        let type_params = self.parse_type_params()?;
        if !self.consume(b'(') {
            return None;
        }
        let mut params = Vec::new();
        while self.pos < self.bytes.len() && self.peek() != Some(b')') {
            params.push(self.parse_type()?);
        }
        self.consume(b')');
        let return_type = self.parse_type()?;
        Some(JvmMethodSignature {
            type_params,
            params,
            return_type,
        })
    }

    fn parse_class_signature(&mut self) -> Option<JvmClassSignature> {
        let type_params = self.parse_type_params()?;
        let mut supertypes = Vec::new();
        if let Some(super_cls) = self.parse_type()
            && super_cls != "Object"
            && super_cls != "Any"
        {
            supertypes.push(super_cls);
        }
        while self.pos < self.bytes.len() {
            if let Some(iface) = self.parse_type() {
                supertypes.push(iface);
            } else {
                break;
            }
        }
        Some(JvmClassSignature {
            type_params,
            supertypes,
        })
    }
}

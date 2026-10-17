pub mod extract;
pub mod functional;
pub mod params;
pub mod raw;
pub mod receiver;
pub mod type_args;
pub mod type_params;

use smol_str::SmolStr;

pub use extract::{extract_param_type_by_index, extract_return_type_from_signature};
pub use functional::{extract_functional_input_types, extract_functional_return_type};
pub use params::{
    find_param_colon, find_parameter_list_bounds, find_parameter_list_open, parse_signature_params,
    parse_single_param,
};
pub use raw::{RawGenericParamInfo, RawSignatureInfo, RawSignatureParam};
pub use receiver::{extract_receiver_type, extract_receiver_type_from_signature};
pub use type_args::{
    extract_type_arguments_list, split_top_level_csv, split_type_arguments, strip_type_annotations,
};
pub use type_params::{extract_param_name, parse_signature_type_params};

pub use crate::syntax::dedent::{
    dedent_and_normalize, dedent_with_base, find_base_indent, strip_comments_from_signature,
    strip_comments_from_signature_with_base,
};

#[inline]
pub fn is_kotlin_modifier(word: &str) -> bool {
    matches!(
        word,
        "private"
            | "public"
            | "protected"
            | "internal"
            | "override"
            | "open"
            | "final"
            | "abstract"
            | "sealed"
            | "data"
            | "inline"
            | "noinline"
            | "crossinline"
            | "value"
            | "infix"
            | "operator"
            | "suspend"
            | "tailrec"
            | "vararg"
            | "external"
            | "annotation"
            | "enum"
            | "inner"
            | "companion"
            | "actual"
            | "expect"
            | "const"
            | "lateinit"
            | "reified"
    )
}

pub fn is_modifier_line(line: &str) -> bool {
    let mut words = line.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    is_kotlin_modifier(first) && words.all(is_kotlin_modifier)
}

/// Parses a raw signature into its constituent tokens without semantic type resolution.
pub fn parse_raw_signature(sig: &str) -> RawSignatureInfo {
    let type_params = parse_signature_type_params(sig);
    let receiver = extract_receiver_type(sig);
    let params = parse_signature_params(sig);
    let return_type = extract_return_type_from_signature(sig).map(SmolStr::new);

    RawSignatureInfo {
        type_params,
        receiver,
        params,
        return_type,
    }
}

use crate::doc::{Doc, IndentKind};

fn dump_leaf(doc: &Doc<'_>, prefix: &str) -> Option<String> {
    match doc {
        Doc::Text(s) => Some(common::concat_str!(
            prefix,
            "Text(\"",
            s.escape_debug().to_string(),
            "\")\n"
        )),
        Doc::Smol(s) => Some(common::concat_str!(
            prefix,
            "Smol(\"",
            s.escape_debug().to_string(),
            "\")\n"
        )),
        Doc::Line => Some(common::concat_str!(prefix, "Line\n")),
        Doc::SoftLine => Some(common::concat_str!(prefix, "SoftLine\n")),
        Doc::HardLine => Some(common::concat_str!(prefix, "HardLine\n")),
        Doc::EmptyLine => Some(common::concat_str!(prefix, "EmptyLine\n")),
        Doc::LineSuffix(s) => Some(common::concat_str!(
            prefix,
            "LineSuffix(\"",
            s.escape_debug().to_string(),
            "\")\n"
        )),
        _ => None,
    }
}

fn dump_branches(doc: &Doc<'_>, indent: usize, prefix: &str) -> Option<String> {
    match doc {
        Doc::IndentIfBreak {
            group_id,
            kind,
            doc,
        } => {
            let id_str = group_id.map_or(String::from("None"), |g| g.0.to_string());
            let kind_str = match kind {
                IndentKind::Block => "Block",
                IndentKind::Continuation => "Continuation",
            };
            Some(common::concat_str!(
                prefix,
                "IndentIfBreak(id=",
                id_str,
                ", kind=",
                kind_str,
                ")(\n",
                dump(doc, indent + 2),
                prefix,
                ")\n"
            ))
        }
        Doc::IfBreak {
            group_id,
            broken,
            flat,
        } => {
            let id_str = group_id.map_or(String::from("None"), |g| g.0.to_string());
            Some(common::concat_str!(
                prefix,
                "IfBreak(id=",
                id_str,
                ") {\n",
                prefix,
                "  broken:\n",
                dump(broken, indent + 4),
                prefix,
                "  flat:\n",
                dump(flat, indent + 4),
                prefix,
                "}\n"
            ))
        }
        Doc::LineComment {
            text,
            is_trailing,
            align_to_start_col,
        } => Some(common::concat_str!(
            prefix,
            "LineComment(trailing=",
            is_trailing.to_string(),
            ", align=",
            align_to_start_col.to_string(),
            ", \"",
            text.escape_debug().to_string(),
            "\")\n"
        )),
        _ => None,
    }
}

pub fn dump(doc: &Doc<'_>, indent: usize) -> String {
    let prefix = " ".repeat(indent);
    if let Some(leaf) = dump_leaf(doc, &prefix) {
        return leaf;
    }
    if let Some(branch) = dump_branches(doc, indent, &prefix) {
        return branch;
    }
    match doc {
        Doc::Indent(inner) => {
            common::concat_str!(prefix, "Indent(\n", dump(inner, indent + 2), prefix, ")\n")
        }
        Doc::ContinuationIndent(inner) => {
            common::concat_str!(
                prefix,
                "ContinuationIndent(\n",
                dump(inner, indent + 2),
                prefix,
                ")\n"
            )
        }
        Doc::Group { id, doc: inner } => {
            let id_str = match id {
                Some(gid) => common::concat_str!("Group#", gid.0.to_string()),
                None => String::from("Group"),
            };
            common::concat_str!(
                prefix,
                id_str,
                "(\n",
                dump(inner, indent + 2),
                prefix,
                ")\n"
            )
        }
        Doc::FitsFirstLine(inner) => {
            common::concat_str!(
                prefix,
                "FitsFirstLine(\n",
                dump(inner, indent + 2),
                prefix,
                ")\n"
            )
        }
        Doc::PreserveTrailing(inner) => {
            common::concat_str!(
                prefix,
                "PreserveTrailing(\n",
                dump(inner, indent + 2),
                prefix,
                ")\n"
            )
        }
        Doc::Concat(items) => {
            let mut out = common::concat_str!(prefix, "Concat([\n");
            for item in items {
                out.push_str(&dump(item, indent + 2));
            }
            out.push_str(&prefix);
            out.push_str("])\n");
            out
        }
        _ => String::new(),
    }
}

use crate::doc::Doc;

pub(crate) fn strip_leading_space<'a>(doc: Doc<'a>) -> Doc<'a> {
    match doc {
        Doc::Concat(mut items) if items.first() == Some(&Doc::space()) => {
            items.remove(0);
            Doc::Concat(items)
        }
        other => other,
    }
}

pub(crate) fn format_property_without_init<'a>(
    name_doc: Doc<'a>,
    type_docs: Vec<Doc<'a>>,
) -> Doc<'a> {
    if type_docs.is_empty() {
        Doc::Group(Box::new(name_doc))
    } else {
        Doc::Group(Box::new(Doc::Concat(vec![
            name_doc,
            Doc::text(":"),
            Doc::continuation_indent(Doc::Concat(vec![Doc::Line, Doc::Concat(type_docs)])),
        ])))
    }
}

pub(crate) fn format_untyped_property_header<'a>(
    name_doc: Doc<'a>,
    op: &'static str,
    init_section: Doc<'a>,
    flat_init: bool,
    first_line_fits: bool,
) -> Doc<'a> {
    if flat_init && !first_line_fits {
        Doc::Concat(vec![
            name_doc,
            Doc::space(),
            Doc::text(op),
            Doc::continuation_indent(Doc::Concat(vec![
                Doc::HardLine,
                strip_leading_space(init_section),
            ])),
        ])
    } else {
        let init_group = if flat_init {
            init_section
        } else {
            Doc::Group(Box::new(init_section))
        };
        Doc::Concat(vec![name_doc, Doc::space(), Doc::text(op), init_group])
    }
}

pub(crate) fn format_flat_property_header<'a>(
    name_doc: Doc<'a>,
    type_docs: Vec<Doc<'a>>,
    op: &'static str,
    init_section: Doc<'a>,
    first_line_fits: bool,
) -> Doc<'a> {
    if first_line_fits {
        Doc::Concat(vec![
            name_doc,
            Doc::text(": "),
            Doc::Concat(type_docs),
            Doc::space(),
            Doc::text(op),
            init_section,
        ])
    } else {
        let type_and_rest = Doc::continuation_indent(Doc::Concat(vec![
            Doc::Line,
            Doc::Concat(type_docs),
            Doc::space(),
            Doc::text(op),
            init_section,
        ]));
        Doc::Group(Box::new(Doc::Concat(vec![
            name_doc,
            Doc::text(":"),
            type_and_rest,
        ])))
    }
}

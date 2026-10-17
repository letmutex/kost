use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};

pub fn sort_by_label(items: &mut Vec<CompletionItem>, is_source: &mut Vec<bool>) {
    let mut combined: Vec<(CompletionItem, bool)> =
        items.drain(..).zip(is_source.drain(..)).collect();
    combined.sort_by(|(a, a_src), (b, b_src)| {
        a.label
            .cmp(&b.label)
            .then_with(|| b_src.cmp(a_src))
            .then_with(|| b.documentation.is_some().cmp(&a.documentation.is_some()))
    });
    for (item, src) in combined {
        items.push(item);
        is_source.push(src);
    }
}

pub fn sort_members(items: &mut Vec<CompletionItem>, is_source: &mut Vec<bool>) {
    let mut combined: Vec<(CompletionItem, bool)> =
        items.drain(..).zip(is_source.drain(..)).collect();
    let rank = |k: Option<CompletionItemKind>| match k {
        Some(CompletionItemKind::PROPERTY | CompletionItemKind::ENUM_MEMBER) => 0,
        Some(CompletionItemKind::METHOD | CompletionItemKind::FUNCTION) => 1,
        _ => 2,
    };
    combined.sort_by(|(a, a_src), (b, b_src)| {
        rank(a.kind)
            .cmp(&rank(b.kind))
            .then_with(|| a.label.cmp(&b.label))
            .then_with(|| b_src.cmp(a_src))
            .then_with(|| b.documentation.is_some().cmp(&a.documentation.is_some()))
    });
    for (item, src) in combined {
        items.push(item);
        is_source.push(src);
    }
}

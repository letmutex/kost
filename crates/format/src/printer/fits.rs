use ahash::AHashMap;

use super::state::{Mode, PrintItem};
use crate::doc::{Doc, GroupId, IndentKind};
use crate::options::FormattingOptions;

#[derive(Debug, Clone)]
pub struct FitItem<'d, 'a> {
    pub indent: usize,
    pub mode: Mode,
    pub doc: &'d Doc<'a>,
    pub in_target_group: bool,
    pub stop_on_break: bool,
}

fn account_text_span(s: &str, mut max_col: isize) -> Option<isize> {
    if s.contains('\n') {
        return None;
    }
    let len = if s.is_ascii() {
        s.len()
    } else {
        s.chars().count()
    };
    max_col -= len as isize;
    if max_col < 0 { None } else { Some(max_col) }
}

fn match_fit_conditional<'d, 'a>(
    item: FitItem<'d, 'a>,
    options: &FormattingOptions,
    group_modes: &AHashMap<GroupId, Mode>,
    scratch: &mut Vec<FitItem<'d, 'a>>,
) {
    match item.doc {
        Doc::IndentIfBreak {
            group_id,
            kind,
            doc: inner,
        } => {
            let is_broken = group_id
                .and_then(|gid| group_modes.get(&gid).copied())
                .unwrap_or(item.mode)
                == Mode::Broken;
            let extra = if is_broken {
                match kind {
                    IndentKind::Block => options.block_indent,
                    IndentKind::Continuation => options.continuation_indent,
                }
            } else {
                0
            };
            scratch.push(FitItem {
                indent: item.indent + extra,
                doc: inner.as_ref(),
                ..item
            });
        }
        Doc::IfBreak {
            group_id,
            broken,
            flat,
        } => {
            let is_broken = group_id
                .and_then(|gid| group_modes.get(&gid).copied())
                .unwrap_or(item.mode)
                == Mode::Broken;
            let next_doc = if is_broken {
                broken.as_ref()
            } else {
                flat.as_ref()
            };
            scratch.push(FitItem {
                doc: next_doc,
                ..item
            });
        }
        _ => {}
    }
}

fn match_fit_structural<'d, 'a>(
    item: FitItem<'d, 'a>,
    options: &FormattingOptions,
    group_modes: &AHashMap<GroupId, Mode>,
    scratch: &mut Vec<FitItem<'d, 'a>>,
) {
    match item.doc {
        Doc::Indent(inner) => {
            scratch.push(FitItem {
                indent: item.indent + options.block_indent,
                mode: if item.stop_on_break && matches!(inner.as_ref(), Doc::Concat(_)) {
                    Mode::Broken
                } else {
                    item.mode
                },
                doc: inner.as_ref(),
                ..item
            });
        }
        Doc::ContinuationIndent(inner) => {
            scratch.push(FitItem {
                indent: item.indent + options.continuation_indent,
                doc: inner.as_ref(),
                ..item
            });
        }
        Doc::Group { doc: inner, .. } => {
            let m = if item.in_target_group {
                Mode::Flat
            } else {
                Mode::Broken
            };
            scratch.push(FitItem {
                mode: m,
                doc: inner.as_ref(),
                ..item
            });
        }
        Doc::FitsFirstLine(inner) => {
            let is_at_end_of_target_group = !scratch.iter().any(|it| it.in_target_group);
            scratch.push(FitItem {
                doc: inner.as_ref(),
                mode: Mode::Flat,
                stop_on_break: item.stop_on_break || is_at_end_of_target_group,
                ..item
            });
        }
        Doc::PreserveTrailing(inner) => {
            scratch.push(FitItem {
                doc: inner.as_ref(),
                ..item
            });
        }
        _ => match_fit_conditional(item, options, group_modes, scratch),
    }
}

enum FitStep {
    Continue,
    Broken,
    LineEnded,
}

fn match_fit_doc<'d, 'a>(
    item: FitItem<'d, 'a>,
    max_col: &mut isize,
    options: &FormattingOptions,
    group_modes: &AHashMap<GroupId, Mode>,
    scratch: &mut Vec<FitItem<'d, 'a>>,
) -> FitStep {
    match item.doc {
        Doc::Text(s) => match account_text_span(s, *max_col) {
            Some(rem) => *max_col = rem,
            None => return FitStep::Broken,
        },
        Doc::Smol(s) => match account_text_span(s, *max_col) {
            Some(rem) => *max_col = rem,
            None => return FitStep::Broken,
        },
        Doc::Line => {
            if item.mode == Mode::Flat {
                *max_col -= 1;
            } else if item.stop_on_break {
                return FitStep::LineEnded;
            } else if item.in_target_group {
                return FitStep::Broken;
            } else {
                return FitStep::LineEnded;
            }
        }
        Doc::SoftLine => {
            if item.mode != Mode::Flat {
                if item.stop_on_break {
                    return FitStep::LineEnded;
                }
                if item.in_target_group {
                    return FitStep::Broken;
                } else {
                    return FitStep::LineEnded;
                }
            }
        }
        Doc::HardLine | Doc::EmptyLine => {
            if item.stop_on_break {
                return FitStep::LineEnded;
            }
            if item.in_target_group {
                return FitStep::Broken;
            } else {
                return FitStep::LineEnded;
            }
        }
        Doc::LineComment { text, .. } => {
            let len = if text.is_ascii() {
                text.len()
            } else {
                text.chars().count()
            };
            *max_col -= (len + 1) as isize;
            if item.stop_on_break || !item.in_target_group {
                return FitStep::LineEnded;
            }
        }
        Doc::LineSuffix(text) => {
            let len = if text.is_ascii() {
                text.len()
            } else {
                text.chars().count()
            };
            *max_col -= (len + 1) as isize;
        }
        Doc::Concat(items) => {
            for it in items.iter().rev() {
                scratch.push(FitItem { doc: it, ..item });
            }
        }
        _ => match_fit_structural(item, options, group_modes, scratch),
    }
    FitStep::Continue
}

pub fn fits<'d, 'a>(
    doc: &'d Doc<'a>,
    stack: &[PrintItem<'d, 'a>],
    group_modes: &AHashMap<GroupId, Mode>,
    mut max_col: isize,
    start_indent: usize,
    options: &FormattingOptions,
    scratch: &mut Vec<FitItem<'d, 'a>>,
) -> bool {
    scratch.clear();
    scratch.push(FitItem {
        indent: start_indent,
        mode: Mode::Flat,
        doc,
        in_target_group: true,
        stop_on_break: false,
    });

    let mut stack_iter = stack.iter().rev();

    while let Some(item) = scratch.pop() {
        if max_col < 0 {
            return false;
        }
        match match_fit_doc(item, &mut max_col, options, group_modes, scratch) {
            FitStep::Continue => {}
            FitStep::Broken => return false,
            FitStep::LineEnded => break,
        }
        if scratch.is_empty()
            && let Some(top) = stack_iter.next()
        {
            scratch.push(FitItem {
                indent: top.indent,
                mode: top.mode,
                doc: top.doc,
                in_target_group: false,
                stop_on_break: false,
            });
        }
    }

    max_col >= 0
}

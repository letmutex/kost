use tower_lsp::lsp_types::{Position, Range};
use tree_sitter::{Node, Point};

/// Convert tree-sitter Point to LSP Position
#[inline]
pub fn point_to_position(point: Point) -> Position {
    Position {
        line: point.row as u32,
        character: point.column as u32,
    }
}

/// Convert tree-sitter node range to LSP Range
#[inline]
pub fn node_to_range(node: &Node) -> Range {
    Range {
        start: point_to_position(node.start_position()),
        end: point_to_position(node.end_position()),
    }
}

/// Convert LSP Position to tree-sitter Point
pub fn position_to_point(pos: Position) -> Point {
    Point {
        row: pos.line as usize,
        column: pos.character as usize,
    }
}

/// Find the greatest character boundary less than or equal to offset
#[inline]
pub fn floor_char_boundary(source: &str, mut offset: usize) -> usize {
    if offset >= source.len() {
        return source.len();
    }
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// Convert LSP Position to byte offset in source string
pub fn position_to_byte_offset(pos: Position, source: &str) -> usize {
    let mut line_start_byte = 0;
    for (current_line, line) in source.split_inclusive('\n').enumerate() {
        if current_line == pos.line as usize {
            let mut utf16_col = 0;
            let mut byte_col = 0;
            let target_col = pos.character as usize;
            for ch in line.chars() {
                if ch == '\r' || ch == '\n' {
                    break;
                }
                let ch_utf16 = ch.len_utf16();
                if utf16_col + ch_utf16 > target_col {
                    break;
                }
                utf16_col += ch_utf16;
                byte_col += ch.len_utf8();
            }
            let raw_offset = line_start_byte + byte_col;
            return floor_char_boundary(source, raw_offset.min(source.len()));
        }
        line_start_byte += line.len();
    }
    source.len()
}

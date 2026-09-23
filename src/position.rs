//! Source position mapping utilities.

/// A 1-indexed line and byte-column position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// Builds a table of byte offsets where each line starts.
///
/// The first entry is always `0`.
pub fn build_line_start_table(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, c) in source.char_indices() {
        if c == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

/// Maps a byte offset in `source` to a 1-indexed line and byte-column position.
///
/// Returns `None` if `offset` is greater than the source length.
pub fn byte_offset_to_position(source: &str, offset: usize) -> Option<Position> {
    if offset > source.len() {
        return None;
    }
    let line_starts = build_line_start_table(source);
    let line = line_starts.partition_point(|&start| start <= offset);
    let line_start = line_starts[line - 1];
    let column = offset - line_start + 1;
    Some(Position { line, column })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_offset_in_first_line() {
        let source = "if x then\nend";
        let pos = byte_offset_to_position(source, 5).unwrap();
        assert_eq!(pos.line, 1);
        assert_eq!(pos.column, 6);
    }

    #[test]
    fn maps_offset_at_start_of_second_line() {
        let source = "if x then\nend";
        let pos = byte_offset_to_position(source, 10).unwrap();
        assert_eq!(pos.line, 2);
        assert_eq!(pos.column, 1);
    }

    #[test]
    fn returns_none_for_offset_beyond_source() {
        let source = "abc";
        assert!(byte_offset_to_position(source, 4).is_none());
    }

    #[test]
    fn offset_to_line_column_boundaries() {
        let source = "ab\ncd\nef";
        assert_eq!(
            byte_offset_to_position(source, 0),
            Some(Position { line: 1, column: 1 })
        );
        assert_eq!(
            byte_offset_to_position(source, 2),
            Some(Position { line: 1, column: 3 })
        );
        assert_eq!(
            byte_offset_to_position(source, 3),
            Some(Position { line: 2, column: 1 })
        );
        assert_eq!(
            byte_offset_to_position(source, 5),
            Some(Position { line: 2, column: 3 })
        );
        assert_eq!(
            byte_offset_to_position(source, 6),
            Some(Position { line: 3, column: 1 })
        );
        assert_eq!(
            byte_offset_to_position(source, 7),
            Some(Position { line: 3, column: 2 })
        );
        assert_eq!(
            byte_offset_to_position(source, 8),
            Some(Position { line: 3, column: 3 })
        );
        assert_eq!(byte_offset_to_position(source, 9), None);
        assert_eq!(
            byte_offset_to_position("", 0),
            Some(Position { line: 1, column: 1 })
        );
    }
}

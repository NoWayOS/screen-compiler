// SPDX-License-Identifier: MIT
//! Byte offsets to line and column.

use screen_compiler_ir::Loc;

/// Line starts of a source text, for turning parser byte offsets into
/// 1-based line and character columns.
#[derive(Clone, Debug)]
pub struct LineIndex {
    /// Byte offset where each line starts; the first is always 0.
    starts: Vec<usize>,
}

impl LineIndex {
    /// Indexes `text`. Lines end at `\r\n`, `\n` or a lone `\r`, as in
    /// editors and in ECMAScript; the `\r` of a `\r\n` belongs to the line
    /// the pair ends. (U+2028 and U+2029 also end lines in ECMAScript, but
    /// most editors do not break there, so they do not here either.)
    #[must_use]
    pub fn new(text: &str) -> Self {
        let bytes = text.as_bytes();
        let mut starts = vec![0];
        for (i, &byte) in bytes.iter().enumerate() {
            let ends_line =
                byte == b'\n' || (byte == b'\r' && bytes.get(i.saturating_add(1)) != Some(&b'\n'));
            if ends_line {
                starts.push(i.saturating_add(1));
            }
        }
        Self { starts }
    }

    /// The location of byte `offset` in `text` (the text this index was built
    /// from). Offsets past the end, or inside a multi-byte character, are
    /// clamped to the nearest character before them.
    #[must_use]
    pub fn loc(&self, text: &str, offset: usize) -> Loc {
        let offset = floor_char_boundary(text, offset);
        let line_index = self
            .starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.starts.get(line_index).copied().unwrap_or(0);
        let column = text
            .get(line_start..offset)
            .map_or(0, |prefix| prefix.chars().count());
        Loc::new(to_u32_plus_one(line_index), to_u32_plus_one(column))
    }
}

fn floor_char_boundary(text: &str, offset: usize) -> usize {
    let mut offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset = offset.saturating_sub(1);
    }
    offset
}

fn to_u32_plus_one(n: usize) -> u32 {
    u32::try_from(n).map_or(u32::MAX, |n| n.saturating_add(1))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        reason = "test code: a failed check panics, and tests index and compute on known data"
    )]

    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::{Config, RngSeed};

    #[test]
    fn maps_offsets_to_lines_and_character_columns() {
        let text = "ab\ncé d\r\n\nx";
        let index = LineIndex::new(text);
        assert_eq!(index.loc(text, 0), Loc::new(1, 1));
        assert_eq!(index.loc(text, 2), Loc::new(1, 3));
        assert_eq!(index.loc(text, 3), Loc::new(2, 1));
        // "é" is two bytes but one column.
        assert_eq!(index.loc(text, 6), Loc::new(2, 3));
        assert_eq!(index.loc(text, 11), Loc::new(4, 1));
        // Clamped: past the end, and inside "é".
        assert_eq!(index.loc(text, 99), Loc::new(4, 2));
        assert_eq!(index.loc(text, 5), Loc::new(2, 2));
    }

    #[test]
    fn carriage_returns_end_lines_as_editors_and_the_parser_do() {
        let text = "a\rb\rc";
        let index = LineIndex::new(text);
        assert_eq!(
            index.loc(text, 4),
            Loc::new(3, 1),
            "the `c` on the third line"
        );
        // CR, CRLF and LF each end one line; the LF of a CRLF belongs to the
        // line the pair ends.
        let text = "a\rb\r\nc\nd";
        let index = LineIndex::new(text);
        assert_eq!(index.loc(text, 2), Loc::new(2, 1));
        assert_eq!(index.loc(text, 4), Loc::new(2, 3));
        assert_eq!(index.loc(text, 5), Loc::new(3, 1));
        assert_eq!(index.loc(text, 7), Loc::new(4, 1));
    }

    fn config() -> Config {
        Config {
            cases: 256,
            failure_persistence: None,
            rng_seed: RngSeed::Fixed(0x6c69_6e65_7300_0001),
            ..Config::default()
        }
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn agrees_with_a_naive_count(text in "[a-cé\\n\\r]{0,40}", pick in 0usize..64) {
            let index = LineIndex::new(&text);
            let boundaries: Vec<usize> = text.char_indices().map(|(i, _)| i).chain([text.len()]).collect();
            let offset = boundaries[pick % boundaries.len()];
            // CRLF, CR and LF each end a line; the CR of a CRLF is the last
            // character of its line.
            let (mut line, mut column) = (1, 1);
            let bytes = text.as_bytes();
            for (i, c) in text.char_indices().take_while(|&(i, _)| i < offset) {
                let ends_line = c == '\n' || (c == '\r' && bytes.get(i + 1) != Some(&b'\n'));
                if ends_line {
                    line += 1;
                    column = 1;
                } else {
                    column += 1;
                }
            }
            let loc = index.loc(&text, offset);
            prop_assert_eq!((loc.line as usize, loc.column as usize), (line, column));
        }
    }
}

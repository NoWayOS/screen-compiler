// SPDX-License-Identifier: MIT
//! Characters that are not shown as themselves.
//!
//! Control characters, invisible formatting characters (the bidirectional
//! controls, zero-width characters and the like) and the Unicode line and
//! paragraph separators either act on whatever displays them (a terminal, a
//! text layout engine) or show nothing at all. Screen Compiler writes them as
//! `\u{…}` in everything it prints ([`escape_special`]), and subset v0 does not
//! accept them in UI text: the device could not show them as a browser does,
//! and bidirectional controls can make source text read differently from what
//! is rendered.

use std::borrow::Cow;
use std::fmt::Write as _;

/// Why a character is not shown as itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpecialChar {
    /// A control character (Unicode general category Cc): U+0000 to U+001F,
    /// U+007F and U+0080 to U+009F.
    Control,
    /// An invisible formatting character (Unicode general category Cf), such
    /// as U+202E RIGHT-TO-LEFT OVERRIDE or U+200B ZERO WIDTH SPACE.
    Format,
    /// U+2028 LINE SEPARATOR or U+2029 PARAGRAPH SEPARATOR.
    Separator,
}

impl SpecialChar {
    /// A short description with its article, for messages.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::Control => "a control character",
            Self::Format => "an invisible formatting character",
            Self::Separator => "a line or paragraph separator",
        }
    }
}

/// Every format character (general category Cf) of Unicode 15.0, as inclusive
/// ranges in ascending order (170 code points). The table is fixed, so what
/// Screen Compiler accepts never depends on the toolchain's Unicode version.
const FORMAT: [(char, char); 21] = [
    ('\u{ad}', '\u{ad}'),       // SOFT HYPHEN
    ('\u{600}', '\u{605}'),     // ARABIC NUMBER SIGN .. ARABIC NUMBER MARK ABOVE
    ('\u{61c}', '\u{61c}'),     // ARABIC LETTER MARK
    ('\u{6dd}', '\u{6dd}'),     // ARABIC END OF AYAH
    ('\u{70f}', '\u{70f}'),     // SYRIAC ABBREVIATION MARK
    ('\u{890}', '\u{891}'),     // ARABIC POUND MARK ABOVE .. ARABIC PIASTRE MARK ABOVE
    ('\u{8e2}', '\u{8e2}'),     // ARABIC DISPUTED END OF AYAH
    ('\u{180e}', '\u{180e}'),   // MONGOLIAN VOWEL SEPARATOR
    ('\u{200b}', '\u{200f}'),   // ZERO WIDTH SPACE .. RIGHT-TO-LEFT MARK
    ('\u{202a}', '\u{202e}'),   // LEFT-TO-RIGHT EMBEDDING .. RIGHT-TO-LEFT OVERRIDE
    ('\u{2060}', '\u{2064}'),   // WORD JOINER .. INVISIBLE PLUS
    ('\u{2066}', '\u{206f}'),   // LEFT-TO-RIGHT ISOLATE .. NOMINAL DIGIT SHAPES
    ('\u{feff}', '\u{feff}'),   // ZERO WIDTH NO-BREAK SPACE (byte order mark)
    ('\u{fff9}', '\u{fffb}'),   // INTERLINEAR ANNOTATION ANCHOR .. TERMINATOR
    ('\u{110bd}', '\u{110bd}'), // KAITHI NUMBER SIGN
    ('\u{110cd}', '\u{110cd}'), // KAITHI NUMBER SIGN ABOVE
    ('\u{13430}', '\u{1343f}'), // EGYPTIAN HIEROGLYPH VERTICAL JOINER .. END WALLED ENCLOSURE
    ('\u{1bca0}', '\u{1bca3}'), // SHORTHAND FORMAT LETTER OVERLAP .. UP STEP
    ('\u{1d173}', '\u{1d17a}'), // MUSICAL SYMBOL BEGIN BEAM .. END PHRASE
    ('\u{e0001}', '\u{e0001}'), // LANGUAGE TAG
    ('\u{e0020}', '\u{e007f}'), // TAG SPACE .. CANCEL TAG
];

/// Whether `c` is not shown as itself, and why.
#[must_use]
pub fn special_char(c: char) -> Option<SpecialChar> {
    if c.is_control() {
        Some(SpecialChar::Control)
    } else if matches!(c, '\u{2028}' | '\u{2029}') {
        Some(SpecialChar::Separator)
    } else if FORMAT
        .iter()
        .any(|&(first, last)| (first..=last).contains(&c))
    {
        Some(SpecialChar::Format)
    } else {
        None
    }
}

/// `text` with every [special character](special_char) written as `\u{…}`
/// (hexadecimal, as in Rust), so it can be printed to a terminal or a log
/// without acting on it. Other text, including non-ASCII letters and
/// punctuation, is unchanged.
#[must_use]
pub fn escape_special(text: &str) -> Cow<'_, str> {
    if !text.chars().any(|c| special_char(c).is_some()) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len().saturating_add(16));
    for c in text.chars() {
        if special_char(c).is_some() {
            // Writing to a String cannot fail.
            let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
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
    fn classifies_controls_format_characters_and_separators() {
        for c in [
            '\u{0}', '\u{7}', '\t', '\n', '\r', '\u{1b}', '\u{7f}', '\u{85}', '\u{9f}',
        ] {
            assert_eq!(special_char(c), Some(SpecialChar::Control), "{c:?}");
        }
        for c in [
            '\u{ad}',
            '\u{61c}',
            '\u{200b}',
            '\u{200d}',
            '\u{200e}',
            '\u{202a}',
            '\u{202e}',
            '\u{2060}',
            '\u{2066}',
            '\u{2069}',
            '\u{feff}',
            '\u{e0001}',
            '\u{e007f}',
        ] {
            assert_eq!(special_char(c), Some(SpecialChar::Format), "{c:?}");
        }
        for c in ['\u{2028}', '\u{2029}'] {
            assert_eq!(special_char(c), Some(SpecialChar::Separator), "{c:?}");
        }
        for c in [
            'a',
            ' ',
            '~',
            '\u{a0}',
            '\u{e9}',
            '\u{2026}',
            '\u{3000}',
            '\u{20ac}',
            '\u{1f600}',
            '\u{2065}',
            '\u{e0002}',
            '\u{e0080}',
        ] {
            assert_eq!(special_char(c), None, "{c:?}");
        }
    }

    #[test]
    fn the_format_table_is_ascending_and_disjoint() {
        let total: u32 = FORMAT
            .iter()
            .map(|&(first, last)| u32::from(last) - u32::from(first) + 1)
            .sum();
        assert_eq!(total, 170, "Unicode 15.0 has 170 format characters");
        for (first, last) in FORMAT {
            assert!(first <= last);
        }
        for pair in FORMAT.windows(2) {
            assert!(pair[0].1 < pair[1].0, "{pair:?}");
        }
    }

    #[test]
    fn escaping_keeps_ordinary_text_and_escapes_the_rest() {
        let plain = "caf\u{e9} \u{2026} \u{a0}ok";
        assert!(matches!(escape_special(plain), Cow::Borrowed(s) if s == plain));
        assert_eq!(
            escape_special("a\u{1b}[2Jb\r\nc\u{7}d\u{202e}e\u{2028}"),
            "a\\u{1b}[2Jb\\u{d}\\u{a}c\\u{7}d\\u{202e}e\\u{2028}"
        );
    }

    fn config() -> Config {
        Config {
            cases: 512,
            failure_persistence: None,
            rng_seed: RngSeed::Fixed(0x6368_6172_7300_0001),
            ..Config::default()
        }
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn escaped_text_has_no_special_characters(text in "\\PC{0,24}|[\\x00-\\x7f\u{80}-\u{9f}\u{200b}-\u{200f}\u{2028}\u{2029}\u{202a}-\u{202e}\u{feff} a-z]{0,24}") {
            let escaped = escape_special(&text);
            prop_assert!(escaped.chars().all(|c| special_char(c).is_none()));
            if text.chars().all(|c| special_char(c).is_none()) {
                prop_assert_eq!(escaped.as_ref(), text.as_str());
            }
        }
    }
}

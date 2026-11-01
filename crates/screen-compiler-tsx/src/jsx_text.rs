// SPDX-License-Identifier: MIT
//! JSX text and attribute strings, converted exactly as React sees them.
//!
//! The parser hands over JSX text and JSX attribute strings raw. Two JSX
//! rules then apply before React ever sees the text:
//!
//! * **Character references** (`&amp;`, `&#169;`) are decoded, in text and in
//!   attribute strings. Screen Compiler decodes `&amp; &lt; &gt; &quot; &apos;
//!   &nbsp;` and numeric references; any other named reference is an error
//!   rather than a guess. An `&` that does not start a reference stays as is.
//! * **Whitespace** in text is folded line by line: tabs become spaces, spaces
//!   touching a line break are removed, lines left empty are dropped, and the
//!   rest are joined with single spaces (Babel's `cleanJSXElementLiteralChild`).
//!
//! Babel and TypeScript implement these rules slightly differently. Where they
//! would render different text, Screen Compiler reports an error instead of picking
//! one: tabs inside rendered text, non-space whitespace (such as U+00A0) at a
//! line edge, U+2028/U+2029, character references that encode whitespace,
//! and line breaks inside attribute strings (Babel folds a line break and the
//! indentation after it into one space; TypeScript keeps both).
//!
//! Characters that are not shown as themselves — control characters and
//! invisible formatting characters such as bidirectional controls, written
//! raw or as references ([`screen_compiler_ir::special_char`]) — are errors too: the
//! device cannot show them as a browser does, and they can make the source
//! read differently from what is rendered. So converted text and attribute
//! strings never contain one.

use std::fmt;

use screen_compiler_ir::special_char;

/// Why a piece of JSX text cannot be converted exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextError {
    /// `&name;` that is not one of the supported named references.
    UnknownReference {
        /// Byte offset of the `&` in the raw text.
        offset: usize,
        /// The reference as written.
        text: String,
    },
    /// A numeric reference that is not a Unicode scalar value.
    InvalidReference {
        /// Byte offset of the `&` in the raw text.
        offset: usize,
        /// The reference as written.
        text: String,
    },
    /// A reference that encodes a space, tab or line break.
    WhitespaceReference {
        /// Byte offset of the `&` in the raw text.
        offset: usize,
        /// The reference as written.
        text: String,
    },
    /// A tab that would be rendered (Babel: as a space; TypeScript: as a tab).
    Tab {
        /// Byte offset of the tab in the raw text.
        offset: usize,
    },
    /// Whitespace other than space or tab at the edge of a line (Babel keeps
    /// it; TypeScript removes it).
    EdgeWhitespace {
        /// Byte offset of the character in the raw text.
        offset: usize,
        /// The character.
        ch: char,
    },
    /// U+2028 or U+2029 (TypeScript breaks the line there; Babel does not).
    LineSeparator {
        /// Byte offset of the character in the raw text.
        offset: usize,
    },
    /// A line break followed by whitespace in an attribute string (Babel
    /// folds both into one space; TypeScript keeps them).
    AttributeLineBreak {
        /// Byte offset of the line break in the raw text.
        offset: usize,
    },
    /// A control, invisible formatting or separator character, written raw
    /// or as a character reference.
    UnsupportedChar {
        /// Byte offset of the character, or of the `&` of its reference.
        offset: usize,
        /// The character.
        ch: char,
    },
}

impl TextError {
    /// Byte offset of the problem in the raw text.
    #[must_use]
    pub fn offset(&self) -> usize {
        match self {
            Self::UnknownReference { offset, .. }
            | Self::InvalidReference { offset, .. }
            | Self::WhitespaceReference { offset, .. }
            | Self::Tab { offset }
            | Self::EdgeWhitespace { offset, .. }
            | Self::LineSeparator { offset }
            | Self::AttributeLineBreak { offset }
            | Self::UnsupportedChar { offset, .. } => *offset,
        }
    }

    /// Advice on how to fix it.
    #[must_use]
    pub fn help(&self) -> &'static str {
        match self {
            Self::UnknownReference { .. } | Self::InvalidReference { .. } => {
                "write the character itself, or use &amp; &lt; &gt; &quot; &apos; &nbsp; or a numeric reference such as &#169;"
            }
            Self::WhitespaceReference { .. } => {
                "write the whitespace itself, or use a string literal such as {\" \"}"
            }
            Self::Tab { .. } => "use spaces instead of tabs",
            Self::EdgeWhitespace { .. } => {
                "move the character away from the line edge, or use a string literal such as {\"\\u00a0\"}"
            }
            Self::LineSeparator { .. } => "remove the character",
            Self::AttributeLineBreak { .. } => "keep the attribute value on one line",
            Self::UnsupportedChar { .. } => {
                "remove it: text in subset v0 is visible characters and spaces only"
            }
        }
    }
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownReference { text, .. } => {
                write!(f, "`{text}` is not a supported character reference")
            }
            Self::InvalidReference { text, .. } => {
                write!(f, "`{text}` does not encode a valid character")
            }
            Self::WhitespaceReference { text, .. } => write!(
                f,
                "`{text}` encodes whitespace, which JSX compilers fold differently"
            ),
            Self::Tab { .. } => f.write_str(
                "tab in JSX text: Babel renders it as a space, TypeScript keeps the tab",
            ),
            Self::EdgeWhitespace { ch, .. } => write!(
                f,
                "U+{:04X} at the edge of a line of JSX text: Babel keeps it, TypeScript removes it",
                u32::from(*ch)
            ),
            Self::LineSeparator { .. } => f.write_str(
                "U+2028/U+2029 in JSX text: JSX compilers disagree on whether it breaks the line",
            ),
            Self::AttributeLineBreak { .. } => f.write_str(
                "line break in an attribute string: Babel joins the lines with one space, TypeScript keeps the line break and the indentation",
            ),
            Self::UnsupportedChar { ch, .. } => write!(
                f,
                "U+{:04X} is {}, which is not supported in text",
                u32::from(*ch),
                special_char(*ch).map_or("a character", screen_compiler_ir::SpecialChar::describe)
            ),
        }
    }
}

impl std::error::Error for TextError {}

/// The named character references Screen Compiler decodes.
const NAMED_REFERENCES: [(&str, char); 6] = [
    ("amp", '&'),
    ("lt", '<'),
    ("gt", '>'),
    ("quot", '"'),
    ("apos", '\''),
    ("nbsp", '\u{a0}'),
];

/// Whitespace TypeScript trims at line edges but Babel does not (TypeScript's
/// `isWhiteSpaceSingleLine`, minus space and tab).
fn is_ts_only_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0b}' | '\u{0c}' | '\u{a0}' | '\u{85}' | '\u{1680}' | '\u{2000}'
            ..='\u{200b}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

/// Whitespace as JavaScript's `\s` matches it (WhiteSpace and
/// LineTerminator), which is what Babel's attribute-string folding uses.
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0b}' | '\u{0c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

/// Converts a raw JSX attribute string (`name="…"`, no escape processing) to
/// the value React receives: character references decoded.
///
/// # Errors
///
/// Returns a [`TextError`] for a line break followed by whitespace (the
/// compilers disagree), for any control, invisible formatting or separator
/// character, and for the references [`decode_references`] refuses.
pub fn attribute_string(raw: &str) -> Result<String, TextError> {
    let mut chars = raw.char_indices().peekable();
    while let Some((offset, c)) = chars.next() {
        let folded_by_babel = match c {
            '\n' => chars
                .peek()
                .is_some_and(|&(_, next)| is_js_whitespace(next)),
            '\r' => {
                let mut rest = raw.get(offset.saturating_add(1)..).unwrap_or("").chars();
                rest.next() == Some('\n') && rest.next().is_some_and(is_js_whitespace)
            }
            _ => false,
        };
        if folded_by_babel {
            return Err(TextError::AttributeLineBreak { offset });
        }
        if special_char(c).is_some() {
            return Err(TextError::UnsupportedChar { offset, ch: c });
        }
    }
    decode_references(raw)
}

/// Decodes the character references in a JSX attribute string or JSX text.
///
/// # Errors
///
/// Returns a [`TextError`] for unsupported named references, invalid numeric
/// references, and numeric references that encode whitespace or a control,
/// invisible formatting or separator character.
pub fn decode_references(raw: &str) -> Result<String, TextError> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    let mut offset = 0_usize;
    while let Some(amp) = rest.find('&') {
        let (before, from_amp) = rest.split_at_checked(amp).unwrap_or((rest, ""));
        out.push_str(before);
        let amp_offset = offset.saturating_add(amp);
        let after = from_amp.get(1..).unwrap_or("");
        let consumed = match reference(after) {
            Reference::None => {
                out.push('&');
                0
            }
            Reference::Named { name, len } => {
                let text = format!("&{name};");
                match NAMED_REFERENCES.iter().find(|(n, _)| *n == name) {
                    Some((_, c)) => out.push(*c),
                    None => {
                        return Err(TextError::UnknownReference {
                            offset: amp_offset,
                            text,
                        });
                    }
                }
                len
            }
            Reference::Numeric { digits, hex, len } => {
                let text = format!("&#{}{digits};", if hex { "x" } else { "" });
                let value = if digits.len() > 8 {
                    None
                } else {
                    u32::from_str_radix(digits, if hex { 16 } else { 10 }).ok()
                };
                match value.and_then(char::from_u32) {
                    None => {
                        return Err(TextError::InvalidReference {
                            offset: amp_offset,
                            text,
                        });
                    }
                    Some(' ' | '\t' | '\n' | '\r') => {
                        return Err(TextError::WhitespaceReference {
                            offset: amp_offset,
                            text,
                        });
                    }
                    Some(c) if special_char(c).is_some() => {
                        return Err(TextError::UnsupportedChar {
                            offset: amp_offset,
                            ch: c,
                        });
                    }
                    Some(c) => out.push(c),
                }
                len
            }
        };
        let skip = consumed.saturating_add(1);
        rest = from_amp.get(skip..).unwrap_or("");
        offset = amp_offset.saturating_add(skip);
    }
    out.push_str(rest);
    Ok(out)
}

enum Reference<'a> {
    /// Not a character reference: the `&` is literal.
    None,
    /// `&name;`; `len` counts the bytes after the `&`, including the `;`.
    Named { name: &'a str, len: usize },
    /// `&#digits;` or `&#xdigits;`; `len` as above.
    Numeric {
        digits: &'a str,
        hex: bool,
        len: usize,
    },
}

/// Recognises a character reference in the text just after an `&`.
fn reference(after: &str) -> Reference<'_> {
    if let Some(numeric) = after.strip_prefix('#') {
        let (hex, digits_from) = match numeric.strip_prefix('x') {
            Some(hex_digits) => (true, hex_digits),
            None => (false, numeric),
        };
        let digits_len = digits_from
            .bytes()
            .take_while(|b| {
                if hex {
                    b.is_ascii_hexdigit()
                } else {
                    b.is_ascii_digit()
                }
            })
            .count();
        let (digits, tail) = digits_from
            .split_at_checked(digits_len)
            .unwrap_or((digits_from, ""));
        if digits.is_empty() || !tail.starts_with(';') {
            return Reference::None;
        }
        let prefix = if hex { 2 } else { 1 };
        return Reference::Numeric {
            digits,
            hex,
            len: digits_len.saturating_add(prefix).saturating_add(1),
        };
    }
    let name_len = after.bytes().take_while(u8::is_ascii_alphanumeric).count();
    let (name, tail) = after.split_at_checked(name_len).unwrap_or((after, ""));
    if name.is_empty() || !tail.starts_with(';') {
        return Reference::None;
    }
    Reference::Named {
        name,
        len: name_len.saturating_add(1),
    }
}

/// Splits on `\r\n`, `\n` and `\r`, returning each line with its byte offset.
fn split_lines(text: &str) -> Vec<(usize, &str)> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0_usize;
    let mut i = 0_usize;
    while let Some(&b) = bytes.get(i) {
        let break_len = match b {
            b'\n' => 1,
            b'\r' if bytes.get(i.saturating_add(1)) == Some(&b'\n') => 2,
            b'\r' => 1,
            _ => {
                i = i.saturating_add(1);
                continue;
            }
        };
        lines.push((start, text.get(start..i).unwrap_or("")));
        i = i.saturating_add(break_len);
        start = i;
    }
    lines.push((start, text.get(start..).unwrap_or("")));
    lines
}

/// Converts raw JSX text to what React renders. `Ok(None)` means the text
/// renders nothing (React drops the child).
///
/// # Errors
///
/// Returns a [`TextError`] for character references Screen Compiler does not
/// decode, for control, invisible formatting and separator characters, and
/// wherever Babel and TypeScript would render different text.
pub fn jsx_text(raw: &str) -> Result<Option<String>, TextError> {
    check_compilers_agree(raw)?;
    // Tabs and line breaks are JSX whitespace, folded or already reported
    // above; U+2028/U+2029 are reported above too.
    if let Some((offset, ch)) = raw
        .char_indices()
        .find(|&(_, c)| !matches!(c, '\t' | '\n' | '\r') && special_char(c).is_some())
    {
        return Err(TextError::UnsupportedChar { offset, ch });
    }
    let decoded = decode_references(raw)?;
    Ok(fold_whitespace(&decoded))
}

/// Reports the constructs Babel and TypeScript fold differently.
fn check_compilers_agree(raw: &str) -> Result<(), TextError> {
    if let Some((offset, _)) = raw
        .char_indices()
        .find(|&(_, c)| c == '\u{2028}' || c == '\u{2029}')
    {
        return Err(TextError::LineSeparator { offset });
    }
    let lines = split_lines(raw);
    let last = lines.len().saturating_sub(1);
    for (index, &(line_offset, line)) in lines.iter().enumerate() {
        let (first_line, last_line) = (index == 0, index == last);
        let start = if first_line {
            0
        } else {
            line.len()
                .saturating_sub(line.trim_start_matches([' ', '\t']).len())
        };
        let end = if last_line {
            line.len()
        } else {
            line.trim_end_matches([' ', '\t']).len()
        };
        let kept = line.get(start..end.max(start)).unwrap_or("");
        let kept_offset = line_offset.saturating_add(start);
        if let Some(i) = kept.find('\t') {
            return Err(TextError::Tab {
                offset: kept_offset.saturating_add(i),
            });
        }
        if !first_line && let Some(c) = kept.chars().next().filter(|&c| is_ts_only_whitespace(c)) {
            return Err(TextError::EdgeWhitespace {
                offset: kept_offset,
                ch: c,
            });
        }
        if !last_line
            && let Some((i, c)) = kept
                .char_indices()
                .next_back()
                .filter(|&(_, c)| is_ts_only_whitespace(c))
        {
            return Err(TextError::EdgeWhitespace {
                offset: kept_offset.saturating_add(i),
                ch: c,
            });
        }
    }
    Ok(())
}

/// Babel's `cleanJSXElementLiteralChild`, on already-decoded text.
fn fold_whitespace(text: &str) -> Option<String> {
    let lines = split_lines(text);
    let last = lines.len().saturating_sub(1);
    let last_non_empty = lines
        .iter()
        .rposition(|(_, line)| line.chars().any(|c| c != ' ' && c != '\t'))
        .unwrap_or(0);
    let mut out = String::new();
    for (index, (_, line)) in lines.iter().enumerate() {
        let spaced = line.replace('\t', " ");
        let mut trimmed: &str = &spaced;
        if index != 0 {
            trimmed = trimmed.trim_start_matches(' ');
        }
        if index != last {
            trimmed = trimmed.trim_end_matches(' ');
        }
        if !trimmed.is_empty() {
            out.push_str(trimmed);
            if index != last_non_empty {
                out.push(' ');
            }
        }
    }
    (!out.is_empty()).then_some(out)
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

    fn text(raw: &str) -> Option<String> {
        jsx_text(raw).unwrap()
    }

    #[test]
    fn single_line_text_is_kept_as_written() {
        assert_eq!(text("Hello"), Some("Hello".into()));
        assert_eq!(text(" Hello  world "), Some(" Hello  world ".into()));
        assert_eq!(text("   "), Some("   ".into()));
    }

    #[test]
    fn whitespace_touching_line_breaks_is_folded() {
        assert_eq!(text("\n  Hello\n  "), Some("Hello".into()));
        assert_eq!(
            text("\n  Touch Start to begin.\n  Then follow the steps.\n"),
            Some("Touch Start to begin. Then follow the steps.".into())
        );
        assert_eq!(text("a  \n\n   \n  b"), Some("a b".into()));
        assert_eq!(text("a\r\nb\rc"), Some("a b c".into()));
        assert_eq!(text("\n   \n"), None);
        assert_eq!(text(""), None);
    }

    #[test]
    fn leading_space_on_the_first_line_and_trailing_on_the_last_are_kept() {
        assert_eq!(text("  a\nb  "), Some("  a b  ".into()));
    }

    #[test]
    fn character_references_are_decoded() {
        assert_eq!(
            text("Help &amp; accessibility"),
            Some("Help & accessibility".into())
        );
        assert_eq!(
            text("&lt;&gt;&quot;&apos;&nbsp;"),
            Some("<>\"'\u{a0}".into())
        );
        assert_eq!(text("&#169; &#xA9;"), Some("\u{a9} \u{a9}".into()));
    }

    #[test]
    fn an_ampersand_that_is_not_a_reference_stays() {
        assert_eq!(
            text("Terms & conditions"),
            Some("Terms & conditions".into())
        );
        assert_eq!(text("&amp"), Some("&amp".into()));
        assert_eq!(text("&#;&#x;&"), Some("&#;&#x;&".into()));
        assert_eq!(text("&#X41;"), Some("&#X41;".into()));
    }

    #[test]
    fn unknown_or_invalid_references_are_errors() {
        assert_eq!(
            jsx_text("a &copy; b"),
            Err(TextError::UnknownReference {
                offset: 2,
                text: "&copy;".into()
            })
        );
        assert!(matches!(
            jsx_text("&#xD800;"),
            Err(TextError::InvalidReference { .. })
        ));
        assert!(matches!(
            jsx_text("&#1114112;"),
            Err(TextError::InvalidReference { .. })
        ));
        assert!(matches!(
            jsx_text("&#999999999999;"),
            Err(TextError::InvalidReference { .. })
        ));
    }

    #[test]
    fn places_where_compilers_disagree_are_errors() {
        assert_eq!(
            jsx_text("&#32;x"),
            Err(TextError::WhitespaceReference {
                offset: 0,
                text: "&#32;".into()
            })
        );
        assert_eq!(jsx_text("a\tb"), Err(TextError::Tab { offset: 1 }));
        assert_eq!(
            jsx_text("a\n\u{a0}b"),
            Err(TextError::EdgeWhitespace {
                offset: 2,
                ch: '\u{a0}'
            })
        );
        assert_eq!(
            jsx_text("a\u{2028}b"),
            Err(TextError::LineSeparator { offset: 1 })
        );
    }

    #[test]
    fn agreed_cases_near_those_edges_are_fine() {
        // Tabs that are trimmed away, and non-breaking spaces inside a line or
        // written as a reference, fold the same way in both compilers.
        assert_eq!(text("\n\tHello\t\n"), Some("Hello".into()));
        assert_eq!(text("a\u{a0}b"), Some("a\u{a0}b".into()));
        assert_eq!(text("a\n&nbsp;b"), Some("a \u{a0}b".into()));
    }

    #[test]
    fn attribute_strings_decode_references_only() {
        assert_eq!(attribute_string("a\\n &amp; b").unwrap(), "a\\n & b");
        assert_eq!(attribute_string("C:\\dir &lt;").unwrap(), "C:\\dir <");
    }

    #[test]
    fn attribute_strings_that_span_lines_are_errors() {
        // Babel folds a line break and the whitespace after it into one
        // space; TypeScript keeps both.
        assert_eq!(
            attribute_string("First name\n        and last name"),
            Err(TextError::AttributeLineBreak { offset: 10 })
        );
        assert_eq!(
            attribute_string("a\r\n  b"),
            Err(TextError::AttributeLineBreak { offset: 1 })
        );
        assert_eq!(
            attribute_string("a\n\nb"),
            Err(TextError::AttributeLineBreak { offset: 1 })
        );
        // Where the compilers agree, a line break is still not text a
        // single-line field can show.
        assert_eq!(
            attribute_string("a\nb"),
            Err(TextError::UnsupportedChar {
                offset: 1,
                ch: '\n'
            })
        );
    }

    #[test]
    fn control_and_invisible_characters_are_errors_however_they_are_written() {
        // Raw, in text and in attribute strings.
        assert_eq!(
            jsx_text("Total: 10\u{202e}00"),
            Err(TextError::UnsupportedChar {
                offset: 9,
                ch: '\u{202e}'
            })
        );
        assert_eq!(
            jsx_text("a\u{1b}[2Jb"),
            Err(TextError::UnsupportedChar {
                offset: 1,
                ch: '\u{1b}'
            })
        );
        assert_eq!(
            attribute_string("x\u{2066}y"),
            Err(TextError::UnsupportedChar {
                offset: 1,
                ch: '\u{2066}'
            })
        );
        assert_eq!(
            attribute_string("a\tb"),
            Err(TextError::UnsupportedChar {
                offset: 1,
                ch: '\t'
            })
        );
        // As numeric references.
        assert_eq!(
            jsx_text("a&#x202E;b"),
            Err(TextError::UnsupportedChar {
                offset: 1,
                ch: '\u{202e}'
            })
        );
        assert_eq!(
            jsx_text("a&#27;b"),
            Err(TextError::UnsupportedChar {
                offset: 1,
                ch: '\u{1b}'
            })
        );
        assert_eq!(
            attribute_string("&#8203;"),
            Err(TextError::UnsupportedChar {
                offset: 0,
                ch: '\u{200b}'
            })
        );
        assert_eq!(
            jsx_text("&#x2028;"),
            Err(TextError::UnsupportedChar {
                offset: 0,
                ch: '\u{2028}'
            })
        );
        // Visible non-ASCII text is fine.
        assert_eq!(
            text("Caf\u{e9} \u{2014} \u{20ac}5"),
            Some("Caf\u{e9} \u{2014} \u{20ac}5".into())
        );
        assert_eq!(attribute_string("\u{e9}t\u{e9}").unwrap(), "\u{e9}t\u{e9}");
    }

    fn config() -> Config {
        Config {
            cases: 1024,
            failure_persistence: None,
            rng_seed: RngSeed::Fixed(0x6a73_7874_6578_7401),
            ..Config::default()
        }
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn never_panics_and_reports_offsets_inside_the_input(
            raw in "[ab &;#x0-9ampltgnbsp\\t\\n\\r\u{a0}\u{2028}\u{1b}\u{202e}\u{feff}]{0,40}"
        ) {
            match jsx_text(&raw) {
                Ok(Some(folded)) => {
                    prop_assert!(folded.chars().all(|c| special_char(c).is_none()));
                }
                Ok(None) => {}
                Err(e) => prop_assert!(e.offset() < raw.len()),
            }
            match attribute_string(&raw) {
                Ok(value) => prop_assert!(value.chars().all(|c| special_char(c).is_none())),
                Err(e) => prop_assert!(e.offset() < raw.len()),
            }
        }

        #[test]
        fn plain_single_line_text_is_unchanged(raw in "[A-Za-z0-9 ,.!?'-]{1,40}") {
            prop_assert_eq!(text(&raw), Some(raw.clone()));
        }

        #[test]
        fn text_without_references_decodes_to_itself(raw in "[^&]{0,40}") {
            prop_assert_eq!(decode_references(&raw).unwrap(), raw);
        }
    }
}

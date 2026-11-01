// SPDX-License-Identifier: MIT
//! Rust lexical helpers: keywords, string literals and comment text.

/// Rust keywords (strict and reserved, edition 2024). None of them may be
/// used as a plain identifier.
const KEYWORDS: &[&str] = &[
    "abstract", "as", "async", "await", "become", "box", "break", "const", "continue", "crate",
    "do", "dyn", "else", "enum", "extern", "false", "final", "fn", "for", "gen", "if", "impl",
    "in", "let", "loop", "macro", "match", "mod", "move", "mut", "override", "priv", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "try", "type", "typeof",
    "unsafe", "unsized", "use", "virtual", "where", "while", "yield",
];

/// Whether `word` is a Rust keyword.
pub(crate) fn is_keyword(word: &str) -> bool {
    KEYWORDS.contains(&word)
}

/// Whether `text` can be written as a plain (non-raw) Rust identifier: an
/// ASCII letter or `_`, then ASCII letters, digits and `_`; not `_` alone
/// and not a keyword.
pub(crate) fn is_identifier(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && text != "_"
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !is_keyword(text)
}

/// A Rust string literal for `text`.
///
/// The literal is pure printable ASCII: `"` and `\` are escaped, tab, line
/// feed and carriage return use their short escapes, and every other
/// character outside printable ASCII is written as `\u{…}`. No invisible or
/// bidirectional-control character can hide in generated code, and the
/// output does not depend on any Unicode table.
#[must_use]
pub fn string_literal(text: &str) -> String {
    let mut out = String::with_capacity(text.len().saturating_add(2));
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ' '..='~' => out.push(c),
            _ => out.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
        }
    }
    out.push('"');
    out
}

/// A source file name as generated comments and documentation show it: its
/// last path component (after any `/` or `\\`), with every character other
/// than ASCII letters, digits, `.`, `_` and `-` written as `\\u{…}`.
///
/// The label goes into `//` comments and `///` documentation, which rustdoc
/// renders as Markdown with HTML passed through, so it can neither end a
/// comment nor carry markup, links or code spans.
pub(crate) fn file_label(name: &str) -> String {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let mut out = String::with_capacity(last.len());
    for c in last.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '-' => out.push(c),
            _ => out.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
        }
    }
    if out.is_empty() {
        out.push_str("unnamed");
    }
    out
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

    /// Undoes [`string_literal`] (test oracle; accepts only its output form).
    fn unescape(literal: &str) -> String {
        let inner = literal
            .strip_prefix('"')
            .unwrap()
            .strip_suffix('"')
            .unwrap();
        let mut out = String::new();
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match chars.next().unwrap() {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                'u' => {
                    assert_eq!(chars.next(), Some('{'));
                    let hex: String = chars.by_ref().take_while(|&c| c != '}').collect();
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).unwrap()).unwrap());
                }
                other => panic!("unexpected escape \\{other}"),
            }
        }
        out
    }

    #[test]
    fn escapes_quotes_backslashes_and_non_ascii() {
        assert_eq!(string_literal("Welcome"), "\"Welcome\"");
        assert_eq!(string_literal("a \"b\" \\ c"), "\"a \\\"b\\\" \\\\ c\"");
        assert_eq!(string_literal("Caf\u{e9}\n"), "\"Caf\\u{e9}\\n\"");
        assert_eq!(string_literal("\u{202e}evil"), "\"\\u{202e}evil\"");
        assert_eq!(string_literal("it's"), "\"it's\"");
    }

    #[test]
    fn file_labels_cannot_break_out_of_comments_or_carry_markup() {
        assert_eq!(
            file_label("a.tsx\nfn evil() {}"),
            "a.tsx\\u{a}fn\\u{20}evil\\u{28}\\u{29}\\u{20}\\u{7b}\\u{7d}"
        );
        assert_eq!(file_label("screens/details-form.tsx"), "details-form.tsx");
        assert_eq!(file_label("C:\\screens\\welcome_2.tsx"), "welcome_2.tsx");
        assert_eq!(file_label("<b>.tsx"), "\\u{3c}b\\u{3e}.tsx");
        assert_eq!(file_label("dir/"), "unnamed");
    }

    #[test]
    fn keywords_are_recognised() {
        assert!(is_keyword("type") && is_keyword("gen") && is_keyword("self"));
        assert!(!is_keyword("title") && !is_keyword("union"));
    }

    fn config() -> Config {
        Config {
            cases: 1024,
            failure_persistence: None,
            rng_seed: RngSeed::Fixed(0x7275_7374_6c69_7401),
            ..Config::default()
        }
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn string_literals_round_trip_and_are_printable_ascii(text in "\\PC{0,32}|[\\x00-\\x7f]{0,32}") {
            let literal = string_literal(&text);
            prop_assert!(literal.chars().all(|c| (' '..='~').contains(&c)));
            prop_assert_eq!(unescape(&literal), text);
        }

        #[test]
        fn file_labels_hold_only_safe_characters(text in "\\PC{0,32}|[\\x00-\\x7f]{0,32}") {
            let label = file_label(&text);
            prop_assert!(!label.is_empty());
            let safe = |c: char| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '\\' | '{' | '}');
            prop_assert!(label.chars().all(safe), "{}", label);
        }
    }
}

// SPDX-License-Identifier: MIT
//! Names that cross from TSX into generated code.
//!
//! A [`Name`] keeps the identifier exactly as written in the source (for
//! comments and messages) and splits it into lowercase ASCII words, so each
//! code generator can render it in its own case style. Two names clash in
//! generated code exactly when their word lists are equal, which is what the
//! IR checks.

use std::fmt;

/// A validated identifier, split into lowercase ASCII words.
///
/// Accepted characters are ASCII letters, digits, `_` and `-`. Words are split
/// at `_`, `-`, lowercase-to-uppercase and digit-to-uppercase changes, and at
/// the end of an acronym (`HTTPServer` is `http`, `server`). The first word
/// must start with a letter.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    source: String,
    words: Vec<String>,
}

/// Why a string is not a usable [`Name`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameError {
    /// Nothing but separators, or empty.
    Empty,
    /// A character outside ASCII letters, digits, `_` and `-`.
    InvalidChar(char),
    /// The first word starts with a digit.
    StartsWithDigit,
}

impl fmt::Display for NameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("it contains no letters"),
            Self::InvalidChar(c) => write!(
                f,
                "it contains {c:?}; only ASCII letters, digits, `_` and `-` are allowed"
            ),
            Self::StartsWithDigit => f.write_str("it must start with a letter"),
        }
    }
}

impl std::error::Error for NameError {}

impl Name {
    /// Validates `source` and splits it into words.
    ///
    /// # Errors
    ///
    /// Returns a [`NameError`] if `source` has no words, contains a character
    /// other than ASCII letters, digits, `_` and `-`, or starts with a digit.
    pub fn parse(source: &str) -> Result<Self, NameError> {
        let words = split_words(source)?;
        Ok(Self {
            source: source.to_owned(),
            words,
        })
    }

    /// The identifier exactly as written in the source.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The lowercase words, never empty.
    #[must_use]
    pub fn words(&self) -> &[String] {
        &self.words
    }

    /// Whether both names render to the same identifier in every case style.
    #[must_use]
    pub fn same_words(&self, other: &Self) -> bool {
        self.words == other.words
    }

    /// The name with a leading `word` removed, when another word follows it
    /// and starts with a letter, so the result still renders to an identifier
    /// in every case style. The source text is kept. Used to turn
    /// `onContinue` into `continue`; `on_2fa` keeps both words.
    #[must_use]
    pub fn without_leading(&self, word: &str) -> Self {
        let next_starts_with_letter = |rest: &[String]| {
            rest.first()
                .is_some_and(|next| next.starts_with(|c: char| c.is_ascii_alphabetic()))
        };
        match self.words.split_first() {
            Some((first, rest)) if first == word && next_starts_with_letter(rest) => Self {
                source: self.source.clone(),
                words: rest.to_vec(),
            },
            _ => self.clone(),
        }
    }

    /// `snake_case`: lowercase words joined with `_`.
    #[must_use]
    pub fn snake_case(&self) -> String {
        self.words.join("_")
    }

    /// `SCREAMING_SNAKE_CASE`: uppercase words joined with `_`.
    #[must_use]
    pub fn screaming_snake_case(&self) -> String {
        self.words.join("_").to_ascii_uppercase()
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

fn split_words(source: &str) -> Result<Vec<String>, NameError> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous: Option<char> = None;
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        let next = chars.peek().copied();
        if c == '_' || c == '-' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous = Some(c);
            continue;
        }
        if !c.is_ascii_alphanumeric() {
            return Err(NameError::InvalidChar(c));
        }
        if c.is_ascii_uppercase() && !current.is_empty() {
            let after_lower_or_digit =
                previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit());
            let acronym_end = previous.is_some_and(|p| p.is_ascii_uppercase())
                && next.is_some_and(|n| n.is_ascii_lowercase());
            if after_lower_or_digit || acronym_end {
                words.push(std::mem::take(&mut current));
            }
        }
        current.push(c.to_ascii_lowercase());
        previous = Some(c);
    }
    if !current.is_empty() {
        words.push(current);
    }
    match words.first() {
        None => Err(NameError::Empty),
        Some(first) if first.starts_with(|c: char| c.is_ascii_digit()) => {
            Err(NameError::StartsWithDigit)
        }
        Some(_) => Ok(words),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        reason = "test code: a failed check panics, and tests index and compute on known data"
    )]

    use super::*;
    use proptest::prelude::*;
    use proptest::test_runner::{Config, RngSeed};

    fn words(s: &str) -> Vec<String> {
        Name::parse(s).unwrap().words().to_vec()
    }

    #[test]
    fn splits_common_styles() {
        assert_eq!(words("onStart"), ["on", "start"]);
        assert_eq!(words("WelcomeScreen"), ["welcome", "screen"]);
        assert_eq!(words("SPACE_MD"), ["space", "md"]);
        assert_eq!(words("first-name"), ["first", "name"]);
        assert_eq!(words("HTTPServer"), ["http", "server"]);
        assert_eq!(words("space2Xl"), ["space2", "xl"]);
        assert_eq!(words("_private"), ["private"]);
        assert_eq!(words("A"), ["a"]);
    }

    #[test]
    fn renders_case_styles() {
        let n = Name::parse("WelcomeScreen").unwrap();
        assert_eq!(n.snake_case(), "welcome_screen");
        assert_eq!(n.screaming_snake_case(), "WELCOME_SCREEN");
        assert_eq!(n.source(), "WelcomeScreen");
    }

    #[test]
    fn rejects_bad_names() {
        assert_eq!(Name::parse(""), Err(NameError::Empty));
        assert_eq!(Name::parse("__"), Err(NameError::Empty));
        assert_eq!(Name::parse("$el"), Err(NameError::InvalidChar('$')));
        assert_eq!(Name::parse("café"), Err(NameError::InvalidChar('é')));
        assert_eq!(Name::parse("2fa"), Err(NameError::StartsWithDigit));
        assert_eq!(Name::parse("_2fa"), Err(NameError::StartsWithDigit));
    }

    #[test]
    fn same_words_detects_clashes_across_styles() {
        let a = Name::parse("fullName").unwrap();
        let b = Name::parse("full-name").unwrap();
        let c = Name::parse("FULL_NAME").unwrap();
        assert!(a.same_words(&b) && b.same_words(&c));
        assert_ne!(a, b, "the source text still differs");
    }

    #[test]
    fn strips_a_leading_word_only_when_more_follow() {
        let press = Name::parse("onContinue").unwrap().without_leading("on");
        assert_eq!(press.words(), ["continue"]);
        assert_eq!(press.source(), "onContinue");
        let only = Name::parse("on").unwrap().without_leading("on");
        assert_eq!(only.words(), ["on"]);
    }

    #[test]
    fn a_leading_word_stays_when_the_next_word_starts_with_a_digit() {
        // `on_2fa` must not become `2fa`: no case style could render it as an
        // identifier (`pub const 2FA` does not compile).
        for source in ["on_2fa", "on-2fa", "on_1", "ON_2FA"] {
            let name = Name::parse(source).unwrap().without_leading("on");
            assert_eq!(
                name.words().first().map(String::as_str),
                Some("on"),
                "{source}"
            );
            assert!(name.screaming_snake_case().starts_with("ON_"), "{source}");
        }
        let press = Name::parse("onSubmit2").unwrap().without_leading("on");
        assert_eq!(press.words(), ["submit2"]);
    }

    fn config() -> Config {
        Config {
            cases: 512,
            failure_persistence: None,
            rng_seed: RngSeed::Fixed(0x7472_616e_736d_7574),
            ..Config::default()
        }
    }

    proptest! {
        #![proptest_config(config())]

        #[test]
        fn parse_never_panics(s in "\\PC{0,24}") {
            let _ = Name::parse(&s);
        }

        #[test]
        fn accepted_names_render_to_valid_identifiers(s in "[A-Za-z_-][A-Za-z0-9_-]{0,20}") {
            if let Ok(name) = Name::parse(&s) {
                let snake = name.snake_case();
                let screaming = name.screaming_snake_case();
                prop_assert!(snake.starts_with(|c: char| c.is_ascii_lowercase()));
                prop_assert!(snake.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'));
                prop_assert!(!snake.contains("__") && !snake.ends_with('_'));
                prop_assert_eq!(screaming.to_ascii_lowercase(), snake);
                prop_assert!(name.words().iter().all(|w| !w.is_empty()));
            }
        }

        #[test]
        fn names_without_a_leading_word_still_render_to_valid_identifiers(s in "(on|ON|On)?[A-Za-z0-9_-]{0,20}") {
            if let Ok(name) = Name::parse(&s) {
                let stripped = name.without_leading("on");
                let snake = stripped.snake_case();
                prop_assert!(snake.starts_with(|c: char| c.is_ascii_lowercase()), "{} -> {}", s, snake);
                prop_assert!(stripped.screaming_snake_case().starts_with(|c: char| c.is_ascii_uppercase()));
                prop_assert!(!stripped.words().is_empty());
            }
        }

        #[test]
        fn rendering_is_injective(a in "[a-z][A-Za-z0-9_]{0,10}", b in "[a-z][A-Za-z0-9_]{0,10}") {
            let (a, b) = (Name::parse(&a).unwrap(), Name::parse(&b).unwrap());
            prop_assert_eq!(a.same_words(&b), a.snake_case() == b.snake_case());
            prop_assert_eq!(a.same_words(&b), a.screaming_snake_case() == b.screaming_snake_case());
        }
    }
}

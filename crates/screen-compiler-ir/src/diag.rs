// SPDX-License-Identifier: MIT
//! Diagnostics shared by every stage: positions, codes and rendering.

use std::fmt;

use crate::escape_special;

/// A position in a source file: which file, 1-based line and 1-based
/// column.
///
/// File 0 is the screen's own file; a file it imports, directly or through
/// another, has the number it was given when first read (see
/// `Module::imported`). Columns count Unicode scalar values (characters),
/// not bytes, so they match what an editor shows for a UTF-8 file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Loc {
    /// The file: 0 for the screen's own.
    pub file: u16,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column, in characters.
    pub column: u32,
}

impl Loc {
    /// A location in the screen's own file from a 1-based line and column.
    #[must_use]
    pub const fn new(line: u32, column: u32) -> Self {
        Self {
            file: 0,
            line,
            column,
        }
    }

    /// The same line and column in file `file`.
    #[must_use]
    pub const fn in_file(self, file: u16) -> Self {
        Self { file, ..self }
    }
}

/// `LINE:COLUMN`; the file is named by whoever renders it.
impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// How serious a diagnostic is. Any error stops conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The input cannot be converted exactly; no output is produced.
    Error,
    /// Worth knowing, but the conversion is still exact.
    Warning,
}

impl Severity {
    /// The lowercase word used when rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

/// A stable code for each kind of diagnostic, rendered as `error[code]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Code {
    /// The input is not valid TSX.
    Syntax,
    /// A language construct outside the supported subset.
    Unsupported,
    /// A JSX element that is not a supported component.
    UnknownElement,
    /// A prop the component does not support.
    UnknownProp,
    /// A prop or constant has a value of the wrong kind or out of range.
    InvalidValue,
    /// A required prop is missing.
    MissingProp,
    /// An element has children it cannot have.
    InvalidChildren,
    /// A name that does not resolve to anything usable.
    UnknownName,
    /// Two things would get the same name in the IR or in generated code.
    DuplicateName,
    /// A name that the code generator reserves or that is a keyword.
    ReservedName,
    /// A size, depth or count limit was exceeded.
    Limit,
    /// The file contains no screen component.
    NoScreen,
    /// Screen Compiler itself failed; please report it.
    Internal,
}

impl Code {
    /// The stable, kebab-case code string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Syntax => "syntax",
            Self::Unsupported => "unsupported",
            Self::UnknownElement => "unknown-element",
            Self::UnknownProp => "unknown-prop",
            Self::InvalidValue => "invalid-value",
            Self::MissingProp => "missing-prop",
            Self::InvalidChildren => "invalid-children",
            Self::UnknownName => "unknown-name",
            Self::DuplicateName => "duplicate-name",
            Self::ReservedName => "reserved-name",
            Self::Limit => "limit",
            Self::NoScreen => "no-screen",
            Self::Internal => "internal",
        }
    }
}

/// One finding about the input, with its position when it has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Error or warning.
    pub severity: Severity,
    /// What kind of finding this is.
    pub code: Code,
    /// Where in the source file, when known.
    pub loc: Option<Loc>,
    /// One sentence saying what is wrong.
    pub message: String,
    /// Optional advice on how to fix it.
    pub help: Option<String>,
}

impl Diagnostic {
    /// An error at `loc`.
    #[must_use]
    pub fn error(code: Code, loc: Option<Loc>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            loc,
            message: message.into(),
            help: None,
        }
    }

    /// Adds advice on how to fix the problem.
    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Whether this diagnostic stops conversion.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Renders `FILE:LINE:COLUMN: error[code]: message`, plus a `help:` line.
    ///
    /// Messages quote the source and `file` comes from the command line, so
    /// every control, invisible formatting or separator character in them is
    /// written as `\u{…}` ([`escape_special`]): a rendered diagnostic is
    /// always exactly one line, or two with help, and cannot act on the
    /// terminal that shows it.
    #[must_use]
    pub fn render(&self, file: &str) -> String {
        self.render_in(&[file])
    }

    /// As [`render`](Self::render), naming the file the location is in:
    /// `files[n]` for file `n`, the screen's own (`files[0]`) for one the
    /// list does not have.
    #[must_use]
    pub fn render_in(&self, files: &[&str]) -> String {
        let file = self
            .loc
            .and_then(|loc| files.get(usize::from(loc.file)))
            .or(files.first())
            .copied()
            .unwrap_or_default();
        let file = escape_special(file);
        let mut out = match self.loc {
            Some(loc) => format!("{file}:{loc}: "),
            None => format!("{file}: "),
        };
        out.push_str(self.severity.as_str());
        out.push('[');
        out.push_str(self.code.as_str());
        out.push_str("]: ");
        out.push_str(&escape_special(&self.message));
        if let Some(help) = &self.help {
            out.push_str("\n  help: ");
            out.push_str(&escape_special(help));
        }
        out
    }
}

/// Whether any diagnostic in the list is an error.
#[must_use]
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(Diagnostic::is_error)
}

/// Sorts diagnostics into a deterministic order: by position (diagnostics
/// without a position first, then by file, line and column), then code, then
/// message. Exact duplicates are removed.
pub fn sort_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.sort_by(|a, b| {
        (a.loc, a.code, &a.message, &a.help, a.severity)
            .cmp(&(b.loc, b.code, &b.message, &b.help, b.severity))
    });
    diagnostics.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_position_code_and_help() {
        let d = Diagnostic::error(Code::UnknownProp, Some(Loc::new(3, 14)), "no prop `x`")
            .with_help("remove it");
        assert_eq!(
            d.render("screen.tsx"),
            "screen.tsx:3:14: error[unknown-prop]: no prop `x`\n  help: remove it"
        );
    }

    #[test]
    fn renders_the_file_a_location_is_in() {
        let d = Diagnostic::error(
            Code::UnknownProp,
            Some(Loc::new(3, 14).in_file(1)),
            "no prop `x`",
        );
        let files = ["screens/check-in.tsx", "screens/parts/header.tsx"];
        assert_eq!(
            d.render_in(&files),
            "screens/parts/header.tsx:3:14: error[unknown-prop]: no prop `x`"
        );
        // A file the list does not have is shown as the screen's own.
        let d = Diagnostic::error(Code::Limit, Some(Loc::new(1, 1).in_file(7)), "big");
        assert_eq!(
            d.render_in(&files),
            "screens/check-in.tsx:1:1: error[limit]: big"
        );
    }

    #[test]
    fn renders_without_position() {
        let d = Diagnostic::error(Code::NoScreen, None, "no screen");
        assert_eq!(d.render("a.tsx"), "a.tsx: error[no-screen]: no screen");
    }

    #[test]
    fn rendering_escapes_characters_that_act_on_a_terminal() {
        // Source text reaches messages and help; the file name comes from the
        // command line. None of them may move the cursor, clear the screen,
        // set the clipboard (OSC 52) or start a forged line.
        let d = Diagnostic::error(
            Code::InvalidValue,
            Some(Loc::new(2, 3)),
            "id \"\u{1b}]52;c;ZWNobyBwd25lZA==\u{7}\" is bad\r\nt.tsx: 0 errors",
        )
        .with_help("gap=\"16px\u{1b}[2J\u{202e}\"");
        let rendered = d.render("evil\u{1b}[1A\u{2028}.tsx");
        assert_eq!(
            rendered,
            "evil\\u{1b}[1A\\u{2028}.tsx:2:3: error[invalid-value]: id \"\\u{1b}]52;c;ZWNobyBwd25lZA==\\u{7}\" is bad\\u{d}\\u{a}t.tsx: 0 errors\n  help: gap=\"16px\\u{1b}[2J\\u{202e}\""
        );
        assert_eq!(
            rendered.lines().count(),
            2,
            "exactly the message line and the help line"
        );
        assert!(
            rendered
                .chars()
                .all(|c| c == '\n' || crate::special_char(c).is_none())
        );
        // Ordinary non-ASCII text is kept as it is.
        let d = Diagnostic::error(
            Code::Unsupported,
            None,
            "use onPress={\u{2026}} in caf\u{e9}",
        );
        assert_eq!(
            d.render("\u{e9}cran.tsx"),
            "\u{e9}cran.tsx: error[unsupported]: use onPress={\u{2026}} in caf\u{e9}"
        );
    }

    #[test]
    fn sorting_is_by_position_then_code_and_removes_duplicates() {
        let late = Diagnostic::error(Code::Syntax, Some(Loc::new(9, 1)), "late");
        let early = Diagnostic::error(Code::UnknownProp, Some(Loc::new(2, 5)), "early");
        let none = Diagnostic::error(Code::NoScreen, None, "none");
        let mut list = vec![late.clone(), early.clone(), none.clone(), early.clone()];
        sort_diagnostics(&mut list);
        assert_eq!(list, vec![none, early, late]);
    }
}

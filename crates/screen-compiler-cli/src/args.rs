// SPDX-License-Identifier: MIT
//! Command-line arguments, parsed by hand: four subcommands and two
//! options do not justify a parser dependency.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::PathBuf;

/// What to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// `screen-compiler convert IN.tsx [-o OUT.rs] [--pictures NAMES]`
    Convert {
        /// The TSX file to convert.
        input: PathBuf,
        /// Where to write the Rust; standard output when absent.
        output: Option<PathBuf>,
        /// The names of the product's pictures, one a line.
        pictures: Option<PathBuf>,
    },
    /// `screen-compiler check IN.tsx [--pictures NAMES]`
    Check {
        /// The TSX file to check.
        input: PathBuf,
        /// The names of the product's pictures, one a line.
        pictures: Option<PathBuf>,
    },
    /// `screen-compiler audit DIR`
    Audit {
        /// The folder whose `.tsx` files are counted.
        dir: PathBuf,
    },
    /// `screen-compiler subset [--dts]`
    Subset {
        /// TypeScript declarations of the kit rather than JSON.
        dts: bool,
    },
    /// `screen-compiler --help`
    Help,
    /// `screen-compiler --version`
    Version,
}

/// Why the arguments are not usable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgError {
    /// No subcommand given.
    MissingCommand,
    /// A subcommand that does not exist.
    UnknownCommand(String),
    /// An option that does not exist for this subcommand.
    UnknownOption(String),
    /// The input file is missing.
    MissingInput,
    /// An option that needs a value got none.
    MissingValue(&'static str),
    /// More positional arguments than the subcommand takes.
    Unexpected(String),
    /// An option given twice.
    Repeated(&'static str),
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => f.write_str("missing command"),
            Self::UnknownCommand(c) => write!(f, "unknown command `{c}`"),
            Self::UnknownOption(o) => write!(f, "unknown option `{o}`"),
            Self::MissingInput => f.write_str("missing input file"),
            Self::MissingValue(o) => write!(f, "`{o}` needs a value"),
            Self::Unexpected(a) => write!(f, "unexpected argument `{a}`"),
            Self::Repeated(o) => write!(f, "`{o}` given more than once"),
        }
    }
}

impl std::error::Error for ArgError {}

/// Parses the arguments after the program name.
///
/// # Errors
///
/// Returns an [`ArgError`] describing the first problem found.
pub fn parse(args: &[OsString]) -> Result<Command, ArgError> {
    let mut args = args.iter();
    let Some(first) = args.next() else {
        return Err(ArgError::MissingCommand);
    };
    match first.to_str() {
        Some("-h" | "--help" | "help") => Ok(Command::Help),
        Some("-V" | "--version") => Ok(Command::Version),
        Some("convert") => {
            let mut input = None;
            let mut output = None;
            let mut pictures = None;
            while let Some(arg) = args.next() {
                if picture_option(arg, &mut args, &mut pictures)? {
                    continue;
                }
                match arg.to_str() {
                    Some("-h" | "--help") => return Ok(Command::Help),
                    Some("-o" | "--output") => {
                        if output.is_some() {
                            return Err(ArgError::Repeated("--output"));
                        }
                        let value = args.next().ok_or(ArgError::MissingValue("--output"))?;
                        output = Some(PathBuf::from(value));
                    }
                    Some(s) if s.starts_with("--output=") => {
                        if output.is_some() {
                            return Err(ArgError::Repeated("--output"));
                        }
                        // Exactly one prefix: `--output=--output=x` names `--output=x`.
                        let value = s.strip_prefix("--output=").unwrap_or_default();
                        if value.is_empty() {
                            return Err(ArgError::MissingValue("--output"));
                        }
                        output = Some(PathBuf::from(value));
                    }
                    _ => positional(arg, &mut input)?,
                }
            }
            Ok(Command::Convert {
                input: input.ok_or(ArgError::MissingInput)?,
                output,
                pictures,
            })
        }
        Some("check") => {
            let mut input = None;
            let mut pictures = None;
            while let Some(arg) = args.next() {
                if picture_option(arg, &mut args, &mut pictures)? {
                    continue;
                }
                match arg.to_str() {
                    Some("-h" | "--help") => return Ok(Command::Help),
                    _ => positional(arg, &mut input)?,
                }
            }
            Ok(Command::Check {
                input: input.ok_or(ArgError::MissingInput)?,
                pictures,
            })
        }
        Some("audit") => {
            let mut dir = None;
            for arg in args {
                match arg.to_str() {
                    Some("-h" | "--help") => return Ok(Command::Help),
                    _ => positional(arg, &mut dir)?,
                }
            }
            Ok(Command::Audit {
                dir: dir.ok_or(ArgError::MissingInput)?,
            })
        }
        Some("subset") => {
            let mut dts = false;
            for arg in args {
                match arg.to_str() {
                    Some("-h" | "--help") => return Ok(Command::Help),
                    Some("--dts") if dts => return Err(ArgError::Repeated("--dts")),
                    Some("--dts") => dts = true,
                    // `subset` takes no file: `positional` reports an option,
                    // and anything else is unexpected.
                    _ => {
                        positional(arg, &mut None)?;
                        return Err(ArgError::Unexpected(lossy(arg)));
                    }
                }
            }
            Ok(Command::Subset { dts })
        }
        _ => Err(ArgError::UnknownCommand(lossy(first))),
    }
}

/// Takes `--pictures FILE` or `--pictures=FILE` into `pictures`: whether
/// `arg` was that option.
fn picture_option<'a>(
    arg: &OsStr,
    args: &mut impl Iterator<Item = &'a OsString>,
    pictures: &mut Option<PathBuf>,
) -> Result<bool, ArgError> {
    let value = match arg.to_str() {
        Some("--pictures") => args
            .next()
            .map(PathBuf::from)
            .ok_or(ArgError::MissingValue("--pictures"))?,
        Some(s) if s.starts_with("--pictures=") => {
            let value = s.strip_prefix("--pictures=").unwrap_or_default();
            if value.is_empty() {
                return Err(ArgError::MissingValue("--pictures"));
            }
            PathBuf::from(value)
        }
        _ => return Ok(false),
    };
    if pictures.is_some() {
        return Err(ArgError::Repeated("--pictures"));
    }
    *pictures = Some(value);
    Ok(true)
}

/// Takes `arg` as the input file, or reports it.
fn positional(arg: &OsStr, input: &mut Option<PathBuf>) -> Result<(), ArgError> {
    let text = lossy(arg);
    if text.starts_with('-') && text.len() > 1 {
        return Err(ArgError::UnknownOption(text));
    }
    if input.is_some() {
        return Err(ArgError::Unexpected(text));
    }
    *input = Some(PathBuf::from(arg));
    Ok(())
}

fn lossy(arg: &OsStr) -> String {
    arg.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    fn run(list: &[&str]) -> Result<Command, ArgError> {
        parse(&args(list))
    }

    #[test]
    fn convert_with_and_without_output() {
        assert_eq!(
            run(&["convert", "a.tsx"]),
            Ok(Command::Convert {
                input: "a.tsx".into(),
                output: None,
                pictures: None,
            })
        );
        let expected = Ok(Command::Convert {
            input: "a.tsx".into(),
            output: Some("b.rs".into()),
            pictures: None,
        });
        assert_eq!(run(&["convert", "a.tsx", "-o", "b.rs"]), expected);
        assert_eq!(run(&["convert", "-o", "b.rs", "a.tsx"]), expected);
        assert_eq!(run(&["convert", "a.tsx", "--output", "b.rs"]), expected);
        assert_eq!(run(&["convert", "a.tsx", "--output=b.rs"]), expected);
    }

    #[test]
    fn an_output_value_is_taken_verbatim() {
        // Only the one `--output=` prefix is the option; the rest is the path.
        assert_eq!(
            run(&["convert", "a.tsx", "--output=--output=x.rs"]),
            Ok(Command::Convert {
                input: "a.tsx".into(),
                output: Some("--output=x.rs".into()),
                pictures: None,
            })
        );
    }

    #[test]
    fn check_takes_one_input() {
        assert_eq!(
            run(&["check", "a.tsx"]),
            Ok(Command::Check {
                input: "a.tsx".into(),
                pictures: None,
            })
        );
        assert_eq!(
            run(&["check", "a.tsx", "b.tsx"]),
            Err(ArgError::Unexpected("b.tsx".into()))
        );
    }

    #[test]
    fn subset_takes_no_arguments() {
        assert_eq!(run(&["subset"]), Ok(Command::Subset { dts: false }));
        assert_eq!(run(&["subset", "--dts"]), Ok(Command::Subset { dts: true }));
        assert_eq!(
            run(&["subset", "--dts", "--dts"]),
            Err(ArgError::Repeated("--dts"))
        );
        assert_eq!(run(&["subset", "--help"]), Ok(Command::Help));
        assert_eq!(
            run(&["subset", "a.tsx"]),
            Err(ArgError::Unexpected("a.tsx".into()))
        );
        assert_eq!(
            run(&["subset", "--json"]),
            Err(ArgError::UnknownOption("--json".into()))
        );
    }

    #[test]
    fn help_and_version() {
        assert_eq!(run(&["--help"]), Ok(Command::Help));
        assert_eq!(run(&["help"]), Ok(Command::Help));
        assert_eq!(run(&["convert", "--help"]), Ok(Command::Help));
        assert_eq!(run(&["-V"]), Ok(Command::Version));
    }

    #[test]
    fn usage_errors() {
        assert_eq!(run(&[]), Err(ArgError::MissingCommand));
        assert_eq!(
            run(&["build"]),
            Err(ArgError::UnknownCommand("build".into()))
        );
        assert_eq!(run(&["convert"]), Err(ArgError::MissingInput));
        assert_eq!(
            run(&["convert", "a.tsx", "-o"]),
            Err(ArgError::MissingValue("--output"))
        );
        assert_eq!(
            run(&["convert", "a.tsx", "--output="]),
            Err(ArgError::MissingValue("--output"))
        );
        assert_eq!(
            run(&["convert", "a.tsx", "-o", "x", "-o", "y"]),
            Err(ArgError::Repeated("--output"))
        );
        assert_eq!(
            run(&["check", "--fast", "a.tsx"]),
            Err(ArgError::UnknownOption("--fast".into()))
        );
    }

    #[test]
    fn a_lone_dash_is_a_file_name() {
        assert_eq!(
            run(&["check", "-"]),
            Ok(Command::Check {
                input: "-".into(),
                pictures: None
            })
        );
    }
}

// SPDX-License-Identifier: MIT
//! `screen-compiler`: converts a supported subset of React/TSX into Rust for
//! NoWayOS's UI runtime.
//!
//! ```text
//! screen-compiler convert IN.tsx [-o OUT.rs]   write Rust to OUT.rs (or standard output)
//! screen-compiler check IN.tsx                 report what is outside subset v0
//! ```
//!
//! Exit status: 0 success; 1 the input cannot be converted (diagnostics on
//! standard error, nothing written); 2 usage error or a file could not be
//! read or written.
//!
//! Everything printed that comes from the input or the command line (file
//! names, quoted source) has its control and invisible formatting characters
//! written as `\u{…}`, so it cannot act on the terminal.

#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod args;

use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use args::Command;
use screen_compiler_analyse::{Options, analyse};
use screen_compiler_ir::{Code, Diagnostic, Module, escape_special, has_errors};

const USAGE: &str = "\
screen-compiler - converts a supported subset of React/TSX into Rust for NoWayOS's UI runtime

Usage:
  screen-compiler convert <INPUT.tsx> [-o <OUTPUT.rs>]
  screen-compiler check <INPUT.tsx>
  screen-compiler --help | --version

Commands:
  convert   Convert INPUT to Rust, written to OUTPUT or standard output.
            Nothing is written if anything is outside the supported subset.
  check     Report every construct outside the supported subset, with its
            line and column, without writing anything.

Exit status: 0 success, 1 not convertible (see diagnostics), 2 usage or I/O error.
";

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match args::parse(&args) {
        Ok(Command::Help) => emit_stdout(USAGE),
        Ok(Command::Version) => {
            emit_stdout(&format!("screen-compiler {}\n", env!("CARGO_PKG_VERSION")))
        }
        Ok(Command::Convert { input, output }) => convert(&input, output.as_deref()),
        Ok(Command::Check { input }) => check(&input),
        Err(error) => {
            let error = error.to_string();
            eprint_best_effort(&format!(
                "screen-compiler: {}\n\n{USAGE}",
                escape_special(&error)
            ));
            ExitCode::from(2)
        }
    }
}

/// `path` as printed: its control and invisible characters escaped.
fn shown(path: &Path) -> String {
    escape_special(&path.display().to_string()).into_owned()
}

/// The result of running the whole pipeline on one file.
enum Outcome {
    /// Converted: the IR and the generated Rust.
    Converted { module: Module, rust: String },
    /// Not converted: at least one error.
    Rejected,
    /// The file could not be read.
    Unreadable,
}

/// Reads, parses, analyses and generates, printing diagnostics to standard
/// error.
fn run(input: &Path) -> Outcome {
    let display = shown(input);
    let source = match read_source(input) {
        Ok(source) => source,
        Err(ReadError::Io(error)) => {
            let error = error.to_string();
            eprint_best_effort(&format!(
                "screen-compiler: cannot read {display}: {}\n",
                escape_special(&error)
            ));
            return Outcome::Unreadable;
        }
        Err(ReadError::Content(diagnostic)) => {
            report(&display, &[diagnostic]);
            return Outcome::Rejected;
        }
    };
    let source_name = input
        .file_name()
        .map_or_else(|| display.clone(), |n| n.to_string_lossy().into_owned());
    let parsed = match screen_compiler_tsx::parse(&source) {
        Ok(parsed) => parsed,
        Err(diagnostics) => {
            report(&display, &diagnostics);
            return Outcome::Rejected;
        }
    };
    let analysis = analyse(&parsed, &Options::new(source_name));
    report(&display, &analysis.diagnostics);
    let Some(module) = analysis.module else {
        return Outcome::Rejected;
    };
    match screen_compiler_codegen_rust::generate(&module) {
        Ok(rust) => Outcome::Converted { module, rust },
        Err(diagnostics) => {
            report(&display, &diagnostics);
            Outcome::Rejected
        }
    }
}

fn convert(input: &Path, output: Option<&Path>) -> ExitCode {
    let display = shown(input);
    match run(input) {
        Outcome::Converted { rust, .. } => match output {
            Some(path) => match write_whole(path, &rust) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    let error = error.to_string();
                    eprint_best_effort(&format!(
                        "screen-compiler: cannot write {}: {}\n",
                        shown(path),
                        escape_special(&error)
                    ));
                    ExitCode::from(2)
                }
            },
            None => emit_stdout(&rust),
        },
        Outcome::Rejected => {
            eprint_best_effort(&format!("screen-compiler: {display} was not converted\n"));
            ExitCode::from(1)
        }
        Outcome::Unreadable => ExitCode::from(2),
    }
}

fn check(input: &Path) -> ExitCode {
    let display = shown(input);
    match run(input) {
        Outcome::Converted { module, .. } => {
            let nodes: usize = module.screens.iter().map(|s| s.nodes().len()).sum();
            let actions: usize = module.screens.iter().map(|s| s.actions().len()).sum();
            emit_stdout(&format!(
                "{display}: supported ({}, {}, {}, {})\n",
                count(module.screens.len(), "screen"),
                count(nodes, "element"),
                count(module.tokens.len(), "design token"),
                count(actions, "action"),
            ))
        }
        Outcome::Rejected => {
            let _ = emit_stdout(&format!("{display}: not supported\n"));
            ExitCode::from(1)
        }
        Outcome::Unreadable => ExitCode::from(2),
    }
}

fn count(n: usize, what: &str) -> String {
    if n == 1 {
        format!("1 {what}")
    } else {
        format!("{n} {what}s")
    }
}

/// Writes `text` to `path` through a new file beside it that is renamed over
/// `path` only once it is complete and flushed to disk, so `path` ends up
/// either as it was or holding all of `text`, never truncated. (A symbolic
/// link at `path` is replaced by the file, not written through.)
fn write_whole(path: &Path, text: &str) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "the path does not name a file")
    })?;
    let dir = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    };
    let (temp, mut file) = create_beside(dir, name)?;
    let written = file
        .write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            fs::rename(&temp, path)
        });
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

/// Creates a new, uniquely named file `.NAME.screen-compiler-PID-N.tmp` in `dir`.
fn create_beside(dir: &Path, name: &OsStr) -> io::Result<(PathBuf, File)> {
    let mut attempt = 0_u32;
    loop {
        let mut temp_name = OsString::from(".");
        temp_name.push(name);
        temp_name.push(format!(
            ".screen-compiler-{}-{attempt}.tmp",
            std::process::id()
        ));
        let temp = dir.join(temp_name);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists && attempt < 16 => {
                attempt = attempt.saturating_add(1);
            }
            Err(error) => return Err(error),
        }
    }
}

enum ReadError {
    Io(io::Error),
    Content(Diagnostic),
}

/// Reads at most [`screen_compiler_tsx::MAX_SOURCE_BYTES`] (plus one, to detect
/// larger files) and checks the text is UTF-8.
fn read_source(path: &Path) -> Result<String, ReadError> {
    let file = File::open(path).map_err(ReadError::Io)?;
    let limit = u64::try_from(screen_compiler_tsx::MAX_SOURCE_BYTES)
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    let mut bytes = Vec::new();
    file.take(limit)
        .read_to_end(&mut bytes)
        .map_err(ReadError::Io)?;
    if bytes.len() > screen_compiler_tsx::MAX_SOURCE_BYTES {
        return Err(ReadError::Content(
            Diagnostic::error(
                Code::Limit,
                None,
                format!(
                    "the file is larger than {} bytes",
                    screen_compiler_tsx::MAX_SOURCE_BYTES
                ),
            )
            .with_help("split the file: one screen per file is plenty"),
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        ReadError::Content(Diagnostic::error(
            Code::Syntax,
            None,
            "the file is not valid UTF-8",
        ))
    })
}

fn report(display: &str, diagnostics: &[Diagnostic]) {
    let mut text = String::new();
    for diagnostic in diagnostics {
        text.push_str(&diagnostic.render(display));
        text.push('\n');
    }
    if has_errors(diagnostics) {
        let errors = diagnostics.iter().filter(|d| d.is_error()).count();
        text.push_str(&format!("{display}: {}\n", count(errors, "error")));
    }
    eprint_best_effort(&text);
}

/// Writes to standard output; a closed pipe is an error exit, not a panic.
fn emit_stdout(text: &str) -> ExitCode {
    let mut stdout = io::stdout().lock();
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(2),
    }
}

/// Writes to standard error, ignoring failures (there is nowhere left to
/// report them).
fn eprint_best_effort(text: &str) {
    let mut stderr = io::stderr().lock();
    let _ = stderr.write_all(text.as_bytes());
    let _ = stderr.flush();
}

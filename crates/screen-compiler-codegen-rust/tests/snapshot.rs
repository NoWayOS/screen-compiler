// SPDX-License-Identifier: MIT
//! Generated Rust for every fixture under `fixtures/` must match
//! `tests/snapshots/<name>.rs` byte for byte.
//!
//! Set `SCREEN_COMPILER_BLESS=1` to rewrite the snapshots after a deliberate change
//! (`make bless`), then review the diff. The snapshots are also compiled and
//! run by `tests/generated_code.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn snapshots_dir() -> PathBuf {
    manifest_dir().join("tests/snapshots")
}

/// The convertible fixtures: `fixtures/*.tsx` (not the `unsupported/` ones).
fn fixture_names() -> Vec<String> {
    let dir = manifest_dir().join("../../fixtures");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "tsx"))
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no fixtures in {}", dir.display());
    names
}

fn stem(name: &str) -> &str {
    name.strip_suffix(".tsx").unwrap()
}

#[test]
fn generated_rust_matches_the_snapshots() {
    let bless = std::env::var_os("SCREEN_COMPILER_BLESS").is_some();
    let mut failures = Vec::new();
    for name in fixture_names() {
        let (_, actual) = support::convert_fixture(&name);
        let snapshot = snapshots_dir().join(format!("{}.rs", stem(&name)));
        if bless {
            fs::write(&snapshot, &actual).unwrap();
            continue;
        }
        let expected = fs::read_to_string(&snapshot).unwrap_or_default();
        if expected != actual && expected.replace("\r\n", "\n") == actual {
            failures.push(format!(
                "{name}: {} has CRLF line endings; the generator writes LF. The repository's .gitattributes keeps checkouts LF: check `git config core.autocrlf` and re-checkout",
                snapshot.display()
            ));
        } else if expected != actual {
            let first_difference = expected
                .lines()
                .zip(actual.lines())
                .position(|(e, a)| e != a)
                .unwrap_or_else(|| expected.lines().count().min(actual.lines().count()));
            failures.push(format!(
                "{name}: generated Rust differs from {} at line {}\n  expected: {:?}\n  actual:   {:?}",
                snapshot.display(),
                first_difference + 1,
                expected.lines().nth(first_difference).unwrap_or("<end of file>"),
                actual.lines().nth(first_difference).unwrap_or("<end of file>"),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\nIf the change is intended, run `make bless` and review the diff.",
        failures.join("\n")
    );
}

#[test]
fn every_snapshot_has_a_fixture() {
    let fixtures: BTreeSet<String> = fixture_names().iter().map(|n| stem(n).to_owned()).collect();
    let snapshots: BTreeSet<String> = fs::read_dir(snapshots_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(snapshots, fixtures, "stale or missing snapshots");
}

#[test]
fn generated_rust_names_the_file_but_never_its_directory() {
    // However a caller spells the path, only the file name reaches the
    // output. (Cross-process determinism is checked by the command-line
    // tests, which run the `screen-compiler` binary several times.)
    for name in fixture_names() {
        let path = manifest_dir().join("../../fixtures").join(&name);
        let source = fs::read_to_string(&path).unwrap();
        let (_, expected) = support::convert_source(&name, &source);
        assert!(expected.is_ascii(), "{name}: generated Rust is not ASCII");
        for spelling in [
            path.display().to_string(),
            format!("fixtures/{name}"),
            format!("./{name}"),
            format!("C:\\work\\screens\\{name}"),
        ] {
            let (_, actual) = support::convert_source(&spelling, &source);
            assert_eq!(actual, expected, "{spelling}: the path changed the output");
        }
        assert!(
            !expected.contains(env!("CARGO_MANIFEST_DIR")),
            "{name}: an absolute path leaked into the output"
        );
    }
}

#[test]
fn line_endings_do_not_change_the_generated_rust() {
    // A Windows checkout without .gitattributes, or a file saved with CR
    // line endings, converts to exactly the same Rust.
    for name in fixture_names() {
        let source = fs::read_to_string(manifest_dir().join("../../fixtures").join(&name)).unwrap();
        let lf = source.replace("\r\n", "\n");
        let (_, expected) = support::convert_source(&name, &lf);
        for (ending, text) in [
            ("CRLF", lf.replace('\n', "\r\n")),
            ("CR", lf.replace('\n', "\r")),
        ] {
            let (_, actual) = support::convert_source(&name, &text);
            assert_eq!(actual, expected, "{name} with {ending} line endings");
        }
    }
}

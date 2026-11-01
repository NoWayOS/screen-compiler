// SPDX-License-Identifier: MIT
//! Every fixture under `fixtures/unsupported/` must be rejected, with exactly
//! the diagnostics recorded in `tests/snapshots/<name>.txt`.
//!
//! Set `SCREEN_COMPILER_BLESS=1` to rewrite the snapshots after a deliberate change
//! (`make bless`), then review the diff.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use std::fs;
use std::path::{Path, PathBuf};

use screen_compiler_analyse::{Options, analyse};

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures() -> Vec<PathBuf> {
    let dir = manifest_dir().join("../../fixtures/unsupported");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "tsx"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures in {}", dir.display());
    paths
}

/// What `screen-compiler check` reports, rendered with the file name only.
fn rendered_diagnostics(path: &Path) -> String {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let source = fs::read_to_string(path).unwrap();
    let diagnostics = match screen_compiler_tsx::parse(&source) {
        Ok(parsed) => {
            let analysis = analyse(&parsed, &Options::new(name.clone()));
            assert!(
                analysis.module.is_none(),
                "{name} must not convert: an unsupported fixture produced IR"
            );
            analysis.diagnostics
        }
        Err(diagnostics) => diagnostics,
    };
    let mut out = String::new();
    for diagnostic in &diagnostics {
        out.push_str(&diagnostic.render(&name));
        out.push('\n');
    }
    out
}

#[test]
fn unsupported_fixtures_report_exactly_their_snapshots() {
    let bless = std::env::var_os("SCREEN_COMPILER_BLESS").is_some();
    let snapshots = manifest_dir().join("tests/snapshots");
    let mut failures = Vec::new();
    for fixture in fixtures() {
        let stem = fixture.file_stem().unwrap().to_string_lossy().into_owned();
        let snapshot = snapshots.join(format!("{stem}.txt"));
        let actual = rendered_diagnostics(&fixture);
        if bless {
            fs::write(&snapshot, &actual).unwrap();
            continue;
        }
        let expected = fs::read_to_string(&snapshot).unwrap_or_default();
        if expected != actual && expected.replace("\r\n", "\n") == actual {
            failures.push(format!(
                "{}: {} has CRLF line endings; diagnostics are written with LF. The repository's .gitattributes keeps checkouts LF: check `git config core.autocrlf` and re-checkout",
                fixture.display(),
                snapshot.display()
            ));
        } else if expected != actual {
            failures.push(format!(
                "{}: diagnostics differ from {}\n--- expected\n{expected}--- actual\n{actual}",
                fixture.display(),
                snapshot.display()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\nIf the change is intended, run `make bless` and review the diff.",
        failures.join("\n")
    );
}

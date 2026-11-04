// SPDX-License-Identifier: MIT
//! The `screen-compiler` binary, end to end.

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
use std::process::{Command, Output};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixtures() -> PathBuf {
    workspace().join("fixtures")
}

fn screen_compiler(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_screen-compiler"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn snapshot(name: &str) -> String {
    fs::read_to_string(
        workspace()
            .join("crates/screen-compiler-codegen-rust/tests/snapshots")
            .join(name),
    )
    .unwrap()
}

/// A fresh scratch directory for one test.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(test);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn convert_writes_the_snapshot_to_standard_output() {
    let out = screen_compiler(&fixtures(), &["convert", "welcome.tsx"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), snapshot("welcome.rs"));
    assert_eq!(text(&out.stderr), "");
}

#[test]
fn convert_writes_the_output_file() {
    let dir = scratch("convert_writes_the_output_file");
    let output = dir.join("details_form.rs");
    let input = fixtures().join("details-form.tsx");
    let out = screen_compiler(
        &dir,
        &[
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "");
    // The header names the file only, not the path it was given as.
    assert_eq!(
        fs::read_to_string(&output).unwrap(),
        snapshot("details-form.rs")
    );
}

#[test]
fn check_summarises_a_supported_file() {
    let out = screen_compiler(&fixtures(), &["check", "welcome.tsx"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        text(&out.stdout),
        "welcome.tsx: supported (1 screen, 7 elements, 2 design tokens, 2 actions)\n"
    );
}

#[test]
fn check_reports_every_unsupported_construct_with_positions() {
    let dir = fixtures().join("unsupported");
    let out = screen_compiler(&dir, &["check", "web-search.tsx"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "web-search.tsx: not supported\n");
    let expected = fs::read_to_string(
        workspace().join("crates/screen-compiler-analyse/tests/snapshots/web-search.txt"),
    )
    .unwrap();
    assert_eq!(
        text(&out.stderr),
        format!("{expected}web-search.tsx: 12 errors\n")
    );
}

#[test]
fn convert_writes_nothing_when_the_input_is_not_supported() {
    let dir = scratch("convert_writes_nothing");
    let output = dir.join("out.rs");
    let input = fixtures().join("unsupported/web-search.tsx");
    let out = screen_compiler(
        &dir,
        &[
            "convert",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(text(&out.stdout), "");
    assert!(!output.exists(), "no partial output");
    assert!(text(&out.stderr).ends_with("was not converted\n"));
}

#[test]
fn a_screen_reads_the_files_it_imports_from_its_folder_and_names_them() {
    // Converted from the folder, or from above it: the same Rust.
    let out = screen_compiler(&fixtures(), &["convert", "check-in-files.tsx"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), snapshot("check-in-files.rs"));
    let out = screen_compiler(&workspace(), &["convert", "fixtures/check-in-files.tsx"]);
    assert_eq!(text(&out.stdout), snapshot("check-in-files.rs"));

    let dir = scratch("a_screen_reads_the_files_it_imports");
    fs::create_dir_all(dir.join("screens/parts")).unwrap();
    fs::write(
        dir.join("screens/screen.tsx"),
        "import { Column, Screen } from \"nowayos:kit\";\nimport { Card } from \"./parts/card\";\nimport { Secret } from \"../secret\";\nexport function S() { return <Screen><Column><Card /></Column></Screen>; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("screens/parts/card.tsx"),
        "import { Text } from \"nowayos:kit\";\nexport function Card() { return <Text colour=\"red\">Fuel</Text>; }\n",
    )
    .unwrap();
    fs::write(dir.join("secret.tsx"), "export const Secret = 1;\n").unwrap();
    let out = screen_compiler(&dir, &["check", "screens/screen.tsx"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    assert!(
        stderr.contains("screens/screen.tsx:3:1: error[unsupported]: \"../secret\" is outside the screen's folder"),
        "{stderr}"
    );
    assert!(
        stderr.contains("screens/parts/card.tsx:2:39: error[unknown-prop]"),
        "{stderr}"
    );
    assert!(
        stderr.ends_with("screens/screen.tsx: 2 errors\n"),
        "{stderr}"
    );
}

#[test]
fn syntax_errors_have_positions() {
    let dir = scratch("syntax_errors_have_positions");
    fs::write(
        dir.join("broken.tsx"),
        "const A = 1;\nexport const GAP = ;\n",
    )
    .unwrap();
    let out = screen_compiler(&dir, &["check", "broken.tsx"]);
    assert_eq!(out.status.code(), Some(1));
    let stderr = text(&out.stderr);
    assert!(
        stderr.starts_with("broken.tsx:2:20: error[syntax]"),
        "{stderr}"
    );
}

#[test]
fn oversized_and_non_utf8_files_are_rejected() {
    let dir = scratch("oversized_and_non_utf8");
    fs::write(
        dir.join("big.tsx"),
        "x".repeat(screen_compiler_tsx::MAX_SOURCE_BYTES + 1),
    )
    .unwrap();
    let out = screen_compiler(&dir, &["check", "big.tsx"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("error[limit]"));
    fs::write(dir.join("latin1.tsx"), b"const A = \"caf\xe9\";\n").unwrap();
    let out = screen_compiler(&dir, &["check", "latin1.tsx"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("not valid UTF-8"));
}

#[test]
fn usage_and_io_errors_exit_with_2() {
    let dir = fixtures();
    for args in [
        &[][..],
        &["frobnicate"][..],
        &["convert"][..],
        &["convert", "welcome.tsx", "-o"][..],
        &["check", "--fast", "welcome.tsx"][..],
    ] {
        let out = screen_compiler(&dir, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(
            text(&out.stderr).starts_with("screen-compiler: "),
            "{args:?}"
        );
    }
    let out = screen_compiler(&dir, &["check", "missing.tsx"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).starts_with("screen-compiler: cannot read missing.tsx"));
}

/// File names in `dir`, sorted.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Whether `bytes` holds only text that prints as itself: no control
/// characters but line feeds, and none of the invisible characters that
/// reorder or hide text. (Its own list, independent of the code under test.)
fn prints_as_itself(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(bytes).chars().all(|c| {
        c == '\n'
            || !(c.is_control()
                || matches!(
                    c,
                    '\u{61c}'
                        | '\u{200b}'..='\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2060}'..='\u{2069}'
                        | '\u{feff}'
                ))
    })
}

#[test]
fn separate_runs_write_identical_rust_whatever_the_path_spelling() {
    // Every run is a new process, so anything that varies between processes
    // (hash seeds, the working directory, the path as given) would show.
    let dir = scratch("separate_runs");
    let elsewhere = dir.join("elsewhere/screens");
    fs::create_dir_all(&elsewhere).unwrap();
    for name in ["details-form.tsx", "edge-names.tsx", "welcome.tsx"] {
        fs::copy(fixtures().join(name), elsewhere.join(name)).unwrap();
        let expected = snapshot(&name.replace(".tsx", ".rs"));
        let absolute = fixtures().join(name).display().to_string();
        let dotted = format!("./{name}");
        let nested = format!("elsewhere/screens/{name}");
        for (cwd, arg) in [
            (fixtures(), name),
            (fixtures(), dotted.as_str()),
            (dir.clone(), absolute.as_str()),
            (elsewhere.clone(), name),
            (dir.clone(), nested.as_str()),
        ] {
            let out = screen_compiler(&cwd, &["convert", arg]);
            assert_eq!(out.status.code(), Some(0), "{arg}: {}", text(&out.stderr));
            assert_eq!(text(&out.stdout), expected, "{arg} from {}", cwd.display());
        }
    }
}

#[test]
fn an_output_value_is_taken_verbatim() {
    let dir = scratch("output_value_verbatim");
    let input = fixtures().join("welcome.tsx");
    let out = screen_compiler(
        &dir,
        &["convert", input.to_str().unwrap(), "--output=--output=x.rs"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(entries(&dir), ["--output=x.rs"]);
}

#[test]
fn the_output_file_is_replaced_whole_or_not_at_all() {
    let dir = scratch("output_replaced_whole");
    let output = dir.join("out.rs");
    let welcome = fixtures().join("welcome.tsx");
    let welcome = welcome.to_str().unwrap();
    // A longer previous file is replaced, not overwritten in place.
    fs::write(&output, "// previous\n".repeat(1000)).unwrap();
    let out = screen_compiler(&dir, &["convert", welcome, "-o", "out.rs"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(fs::read_to_string(&output).unwrap(), snapshot("welcome.rs"));
    assert_eq!(entries(&dir), ["out.rs"], "no temporary file is left");
    // Refused input leaves the previous file as it was.
    fs::write(&output, "// previous\n").unwrap();
    let unsupported = fixtures().join("unsupported/web-search.tsx");
    let out = screen_compiler(
        &dir,
        &["convert", unsupported.to_str().unwrap(), "-o", "out.rs"],
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(fs::read_to_string(&output).unwrap(), "// previous\n");
    // An output that cannot be written is an I/O error, and nothing is left
    // behind.
    fs::create_dir(dir.join("a-directory")).unwrap();
    let out = screen_compiler(&dir, &["convert", welcome, "-o", "a-directory"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(text(&out.stderr).starts_with("screen-compiler: cannot write a-directory"));
    assert_eq!(entries(&dir), ["a-directory", "out.rs"]);
    assert!(entries(&dir.join("a-directory")).is_empty());
}

#[test]
fn source_text_cannot_act_on_the_terminal() {
    // Each line puts terminal control sequences (cursor movement, screen
    // clearing, an OSC 52 clipboard write, a carriage return and a forged
    // summary line) where a diagnostic quotes the source.
    let dir = scratch("source_text_escaped");
    let source = "\"\u{1b}[2J\u{1b}[H\";\n\
        import \"\u{1b}[2K\\r\u{1b}[1Aok.tsx: supported (1 screen)\\n\";\n\
        import { Column, Screen, Text } from \"nowayos:kit\";\n\
        export function S({ \"\u{1b}]0;owned\u{7}x\": onGo }: P) {\n\
          return <Screen><Column gap={\"16px\u{1b}]52;c;ZWNobyBwd25lZA==\u{7}\"}><Text id=\"\u{1b}[31m\">a\u{202e}b</Text></Column></Screen>;\n\
        }\n";
    fs::write(dir.join("screen.tsx"), source).unwrap();
    let out = screen_compiler(&dir, &["check", "screen.tsx"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(prints_as_itself(&out.stdout), "{:?}", text(&out.stdout));
    assert!(prints_as_itself(&out.stderr), "{:?}", text(&out.stderr));
    // Every line is a diagnostic, its help, or the summary: none is forged.
    let stderr = text(&out.stderr);
    assert!(
        stderr
            .lines()
            .all(|l| l.starts_with("screen.tsx:") || l.starts_with("  help: ")),
        "{stderr}"
    );
    assert!(stderr.contains("\\u{1b}[2K\\u{d}"), "{stderr}");
}

#[cfg(unix)]
#[test]
fn file_names_cannot_act_on_the_terminal() {
    let dir = scratch("file_names_escaped");
    let name = "x\u{1b}]52;c;ZWNobyBwd25lZA==\u{7}\u{1b}[2K\r.tsx";
    fs::write(dir.join(name), "const A = <div />;\n").unwrap();
    let out = screen_compiler(&dir, &["check", name]);
    assert_eq!(out.status.code(), Some(1));
    assert!(prints_as_itself(&out.stdout), "{:?}", text(&out.stdout));
    assert!(prints_as_itself(&out.stderr), "{:?}", text(&out.stderr));
    let out = screen_compiler(&dir, &["convert", name, "-o", "\u{1b}[2J/out.rs"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(prints_as_itself(&out.stderr), "{:?}", text(&out.stderr));
    let out = screen_compiler(&dir, &["frobnicate\u{1b}[2J"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(prints_as_itself(&out.stderr), "{:?}", text(&out.stderr));
}

#[test]
fn help_and_version() {
    let out = screen_compiler(&fixtures(), &["--help"]);
    assert_eq!(out.status.code(), Some(0));
    assert!(text(&out.stdout).contains("screen-compiler convert <INPUT.tsx>"));
    let out = screen_compiler(&fixtures(), &["--version"]);
    assert_eq!(
        text(&out.stdout),
        format!("screen-compiler {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn audit_counts_a_folder_and_quotes_no_code() {
    let out = screen_compiler(&fixtures(), &["audit", "."]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("screen-compiler audit: "), "{stdout}");
    assert!(stdout.contains("  welcome.tsx  converts\n"), "{stdout}");
    // Names, never code: the fixture's search placeholder text is not quoted.
    let source = fs::read_to_string(fixtures().join("unsupported/web-search.tsx")).unwrap();
    assert!(source.contains("useState"));
    assert!(!stdout.contains("=>"), "{stdout}");
    let missing = screen_compiler(&fixtures(), &["audit", "no-such-folder"]);
    assert_eq!(missing.status.code(), Some(2));
}

#[test]
fn subset_prints_the_committed_tables() {
    let out = screen_compiler(&workspace(), &["subset"]);
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let committed = fs::read_to_string(workspace().join("kit/subset.json")).unwrap();
    assert_eq!(text(&out.stdout), committed);
    assert!(out.stderr.is_empty(), "{out:?}");
    let dts = screen_compiler(&workspace(), &["subset", "--dts"]);
    assert_eq!(dts.status.code(), Some(0), "{dts:?}");
    let committed = fs::read_to_string(workspace().join("kit/nowayos-kit.d.ts")).unwrap();
    assert_eq!(text(&dts.stdout), committed);
    let extra = screen_compiler(&workspace(), &["subset", "fixtures"]);
    assert_eq!(extra.status.code(), Some(2));
}

/// `<Image src>` names one of the pictures `--pictures` lists: converted with
/// the list, as its snapshot; refused without it, or for a name outside it;
/// and a list that is not one is a usage error, nothing converted.
#[test]
fn pictures_are_named_from_the_list_given() {
    let dir = fixtures();
    let out = screen_compiler(
        &dir,
        &["convert", "--pictures", "pictures.names", "photo-card.tsx"],
    );
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), snapshot("photo-card.rs"));

    let out = screen_compiler(&dir, &["check", "photo-card.tsx"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("no pictures were given"));

    let scratch = scratch("pictures_are_named_from_the_list_given");
    fs::write(scratch.join("names"), "badge\n").unwrap();
    let names = scratch.join("names");
    let out = screen_compiler(
        &dir,
        &[
            "check",
            "--pictures",
            names.to_str().unwrap(),
            "photo-card.tsx",
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("there is no picture named \"landscape\""));

    fs::write(&names, "Not A Name\n").unwrap();
    let out = screen_compiler(
        &dir,
        &["check", "--pictures=".to_owned().as_str(), "photo-card.tsx"],
    );
    assert_eq!(out.status.code(), Some(2));
    let out = screen_compiler(
        &dir,
        &[
            "check",
            &format!("--pictures={}", names.display()),
            "photo-card.tsx",
        ],
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("is not a picture's name"));
}

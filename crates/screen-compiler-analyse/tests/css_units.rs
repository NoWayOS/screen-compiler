// SPDX-License-Identifier: MIT
//! CSS's own ways of writing what the subset takes: a length in px, the
//! layout units a number is, and a theme colour as `var(--name)` - each
//! exactly what the plain form converts to; other units, and a colour with
//! a fallback, refused where written.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use screen_compiler_analyse::{Analysis, Options, analyse};
use screen_compiler_ir::{Code, Diagnostic, Module, outline};

const KIT: &str = "import { Column, Screen, Spacer, Text } from \"nowayos:kit\";\n";

fn run(source: &str) -> Analysis {
    match screen_compiler_tsx::parse(source) {
        Ok(parsed) => analyse(&parsed, &Options::new("test.tsx")),
        Err(diagnostics) => Analysis {
            module: None,
            diagnostics,
        },
    }
}

fn ok(source: &str) -> Module {
    let analysis = run(source);
    match analysis.module {
        Some(module) => module,
        None => panic!(
            "expected a module, got {:#?}\n{source}",
            analysis.diagnostics
        ),
    }
}

fn ok_outline(source: &str) -> String {
    let module = ok(source);
    outline(&module, &module.screens[0])
}

fn error(source: &str) -> Diagnostic {
    let analysis = run(source);
    assert!(
        analysis.module.is_none(),
        "an error must stop conversion:\n{source}"
    );
    let errors: Vec<Diagnostic> = analysis
        .diagnostics
        .into_iter()
        .filter(Diagnostic::is_error)
        .collect();
    assert_eq!(errors.len(), 1, "{errors:#?}\n{source}");
    errors.into_iter().next().unwrap()
}

fn screen(column: &str) -> String {
    format!("{KIT}export function S() {{ return <Screen>{column}</Screen>; }}\n")
}

#[test]
fn a_length_in_px_is_that_many_layout_units() {
    let numbers = ok_outline(&screen(
        "<Column gap={16} padding={8} style={{ width: 320, minWidth: 0, maxWidth: 640, height: 200 }}><Spacer size={24} /><Text>Hi</Text></Column>",
    ));
    let px = ok_outline(&screen(
        "<Column gap=\"16px\" padding=\"8px\" style={{ width: \"320px\", minWidth: \"0px\", maxWidth: \"640px\", height: \"200px\" }}><Spacer size=\"24px\" /><Text>Hi</Text></Column>",
    ));
    assert_eq!(px, numbers);
    // In a style, gap and padding too.
    assert_eq!(
        ok_outline(&screen(
            "<Column style={{ gap: \"16px\", padding: \"8px\" }}><Text>Hi</Text></Column>"
        )),
        ok_outline(&screen(
            "<Column style={{ gap: 16, padding: 8 }}><Text>Hi</Text></Column>"
        ))
    );
}

#[test]
fn other_units_and_other_strings_are_refused_where_written() {
    for (value, code) in [
        ("1rem", Code::Unsupported),
        ("2em", Code::Unsupported),
        ("50%", Code::Unsupported),
        ("100vh", Code::Unsupported),
        ("10vw", Code::Unsupported),
        ("16", Code::InvalidValue),
        ("16.5px", Code::InvalidValue),
        ("016px", Code::InvalidValue),
        ("-4px", Code::InvalidValue),
        ("65536px", Code::InvalidValue),
        ("px", Code::InvalidValue),
        ("calc(1px + 2px)", Code::InvalidValue),
    ] {
        let e = error(&screen(&format!(
            "<Column gap=\"{value}\"><Text>Hi</Text></Column>"
        )));
        assert_eq!(e.code, code, "{value}: {}", e.message);
        assert_eq!(e.loc.unwrap().column, 50, "{value}: at its opening quote");
    }
}

#[test]
fn a_theme_colour_as_a_css_variable_is_that_colour() {
    let named = ok_outline(&screen(
        "<Column style={{ backgroundColor: \"input\", borderColor: \"input-border\" }}><Text style={{ color: \"text-muted\" }}>Hi</Text></Column>",
    ));
    let variables = ok_outline(&screen(
        "<Column style={{ backgroundColor: \"var(--input)\", borderColor: \"var(--input-border)\" }}><Text style={{ color: \"var(--text-muted)\" }}>Hi</Text></Column>",
    ));
    assert_eq!(variables, named);
    for value in [
        "var(--brand)",
        "var(--text-muted, #666)",
        "var(text)",
        "#666666",
    ] {
        let e = error(&screen(&format!(
            "<Column><Text style={{{{ color: \"{value}\" }}}}>Hi</Text></Column>"
        )));
        assert_eq!(e.code, Code::InvalidValue, "{value}");
    }
}

// SPDX-License-Identifier: MIT
//! Names from TSX reach generated Rust only as identifiers that compile.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use screen_compiler_analyse::{Options, analyse};
use screen_compiler_ir::{Code, Diagnostic};

/// The generated Rust, or every diagnostic.
fn generate(source: &str) -> Result<String, Vec<Diagnostic>> {
    let parsed = screen_compiler_tsx::parse(source)?;
    let analysis = analyse(&parsed, &Options::new("t.tsx"));
    let module = analysis.module.ok_or(analysis.diagnostics)?;
    screen_compiler_codegen_rust::generate(&module)
}

fn is_identifier(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && text != "_"
        && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[test]
fn callback_props_whose_next_word_starts_with_a_digit_keep_the_on() {
    for (prop, constant) in [
        ("on_2fa", "ON_2FA"),
        ("on_1", "ON_1"),
        ("\"on-2fa\": on2", "ON_2FA"),
    ] {
        let local = prop.rsplit(": ").next().unwrap();
        let source = format!(
            "import {{ Button, Screen }} from \"nowayos:kit\";\nexport function S({{ {prop} }}: P) {{ return <Screen><Button onPress={{{local}}}>Verify</Button></Screen>; }}\n"
        );
        let rust = generate(&source).unwrap();
        for line in rust.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("pub const ") {
                let ident = rest.split(':').next().unwrap();
                assert!(
                    is_identifier(ident),
                    "`{ident}` is not an identifier:\n{rust}"
                );
            }
        }
        assert!(
            rust.contains(&format!("pub const {constant}: ui::ActionId")),
            "{rust}"
        );
        assert!(rust.contains(&format!("actions::{constant})?")), "{rust}");
    }
}

#[test]
fn a_screen_named_like_the_api_crate_is_refused() {
    let source = "import { Screen, Text } from \"nowayos:kit\";\nexport function NowayosUi() { return <Screen><Text>Hi</Text></Screen>; }\n";
    let diagnostics = generate(source).unwrap_err();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].code, Code::ReservedName);
}

#[test]
fn a_switch_whose_cases_module_would_be_reserved_is_refused() {
    let screen = |condition: &str| {
        format!(
            "import {{ Column, Screen, Text }} from \"nowayos:kit\";\nexport function S(props) {{ return <Screen><Column>{{{condition} && <Text>A</Text>}}</Column></Screen>; }}\n"
        )
    };
    for prop in ["type", "self", "ui"] {
        let diagnostics = generate(&screen(&format!("props.{prop} === \"a\""))).unwrap_err();
        assert_eq!(diagnostics.len(), 1, "{prop}: {diagnostics:#?}");
        assert_eq!(diagnostics[0].code, Code::ReservedName, "{prop}");
    }
    // Its cases are constants, never keywords.
    let rust = generate(&screen("props.kind === \"type\"")).unwrap();
    assert!(rust.contains("pub const TYPE: u8 = 0;"), "{rust}");
}

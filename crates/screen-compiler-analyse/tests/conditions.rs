// SPDX-License-Identifier: MIT
//! Conditions beyond one prop's truth (contract v13): a prop negated, the
//! group shown while the prop's is hidden, and a prop compared with string
//! literals, one switch the application sets, each literal a case.

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

const KIT: &str = "import { Column, Screen, Text } from \"nowayos:kit\";\n";

fn run(source: &str) -> Analysis {
    match screen_compiler_tsx::parse(source) {
        Ok(parsed) => analyse(&parsed, &Options::new("test.tsx")),
        Err(diagnostics) => Analysis {
            module: None,
            diagnostics,
            imported: Vec::new(),
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

/// A file declaring `top`, its screen's `<Column>` holding `body`, its
/// props the object `props`.
fn file(top: &str, body: &str) -> String {
    format!(
        "{KIT}{top}\nexport function S(props) {{ return <Screen><Column>{body}</Column></Screen>; }}\n"
    )
}

#[test]
fn a_prop_negated_shows_its_content_while_the_prop_s_group_is_hidden() {
    let negated = ok_outline(&file("", "{!props.ready && <Text>Waiting</Text>}"));
    assert!(negated.contains("unless group 1"), "{negated}");
    // `p ? null : <X/>` is the same; `!p ? null : <X/>` is `p && <X/>`.
    assert_eq!(
        ok_outline(&file("", "{props.ready ? null : <Text>Waiting</Text>}")),
        negated
    );
    assert_eq!(
        ok_outline(&file("", "{!props.ready ? null : <Text>Ready</Text>}")),
        ok_outline(&file("", "{props.ready && <Text>Ready</Text>}"))
    );
    // `!p ? <X/> : <Y/>` is `p ? <Y/> : <X/>`.
    assert_eq!(
        ok_outline(&file(
            "",
            "{!props.ready ? <Text>Waiting</Text> : <Text>Ready</Text>}"
        )),
        ok_outline(&file(
            "",
            "{props.ready ? <Text>Ready</Text> : <Text>Waiting</Text>}"
        ))
    );
    // A negation shares the prop's group with the prop itself.
    let module = ok(&file(
        "",
        "{props.ready && <Text>Ready</Text>}{!props.ready && <Text>Waiting</Text>}",
    ));
    assert_eq!(module.screens[0].groups().len(), 1);
}

#[test]
fn a_prop_compared_with_literals_is_one_switch_each_literal_a_case() {
    let source = file(
        "",
        "{props.step === \"choose\" && <Text>Choose</Text>}\
         {props.step === \"review\" && <Text>Review</Text>}\
         {\"pay\" === props.step ? <Text>Pay</Text> : <Text>Not paying yet</Text>}\
         {props.step === \"review\" && <Text>Check it</Text>}",
    );
    let module = ok(&source);
    let screen = &module.screens[0];
    let switches = screen.switches();
    assert_eq!(switches.len(), 1, "one prop, one switch");
    let (switch, cases) = &switches[0];
    assert_eq!(switch.source(), "step");
    let cases: Vec<&str> = cases.iter().map(|case| case.source()).collect();
    assert_eq!(
        cases,
        ["choose", "review", "pay"],
        "in the order first written"
    );
    let outline = outline(&module, screen);
    for line in ["switch 1 case 0", "switch 1 case 1", "switch 1 case 2"] {
        assert!(outline.contains(line), "{line}:\n{outline}");
    }
    assert_eq!(screen.groups().len(), 0, "a switch is not a group");
    // Destructured props likewise.
    let destructured = format!(
        "{KIT}export function S({{ step }}) {{ return <Screen><Column>{{step === \"review\" && <Text>Review</Text>}}</Column></Screen>; }}\n"
    );
    assert_eq!(
        ok_outline(&destructured),
        ok_outline(&file(
            "",
            "{props.step === \"review\" && <Text>Review</Text>}"
        ))
    );
}

#[test]
fn in_a_part_an_omitted_prop_decides_its_conditions_at_build_time() {
    let part = "function Notice({ ready, step, children }) {\n  return <Column>{!ready && <Text>Waiting</Text>}{step === \"pay\" && <Text>Pay</Text>}{children}</Column>;\n}";
    // Omitted, `ready` is `undefined`: `!ready` holds and the step is no case.
    assert_eq!(
        ok_outline(&file(part, "<Notice><Text>Body</Text></Notice>")),
        ok_outline(&file(
            "",
            "<Column><Text>Waiting</Text><Text>Body</Text></Column>"
        ))
    );
    // Given the screen's props, they are the screen's conditions.
    assert_eq!(
        ok_outline(&file(
            part,
            "<Notice ready={props.ready} step={props.step}><Text>Body</Text></Notice>"
        )),
        ok_outline(&file(
            "",
            "<Column>{!props.ready && <Text>Waiting</Text>}{props.step === \"pay\" && <Text>Pay</Text>}<Text>Body</Text></Column>"
        ))
    );
}

#[test]
fn other_comparisons_are_refused_where_written() {
    for (body, says) in [
        (
            "{props.a === props.b && <Text>Same</Text>}",
            "string literal",
        ),
        ("{props.count === 2 && <Text>Two</Text>}", "string literal"),
        ("{\"a\" === \"b\" && <Text>Never</Text>}", "string literal"),
        ("{!!props.ready && <Text>Ready</Text>}", "`!!`"),
        (
            "{props.step === \"pay\" ? null : <Text>Not paying</Text>}",
            "one case",
        ),
    ] {
        let e = error(&file("", body));
        assert_eq!(e.code, Code::Unsupported, "{body}");
        assert!(e.message.contains(says), "{body}: {}", e.message);
    }
    // A literal that cannot name a constant.
    let e = error(&file("", "{props.step === \"\" && <Text>Empty</Text>}"));
    assert_eq!(e.code, Code::InvalidValue);
    // Two literals JavaScript tells apart but generated code would not.
    let e = error(&file(
        "",
        "{props.step === \"review\" && <Text>A</Text>}{props.step === \"Review\" && <Text>B</Text>}",
    ));
    assert_eq!(e.code, Code::DuplicateName);
    assert!(e.message.contains("same name"), "{}", e.message);
}

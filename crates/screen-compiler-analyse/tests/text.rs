// SPDX-License-Identifier: MIT
//! Text that mixes literals and one prop (contract v14): written in JSX or
//! as a template literal, the literals compiled in around a slot the
//! application fills - and every other mix refused where it is written.

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
fn literals_around_one_prop_are_compiled_in_and_the_prop_is_a_slot() {
    let hello = ok_outline(&file("", "<Text>Hello, {props.name}!</Text>"));
    assert!(
        hello.contains("slot 1 between \"Hello, \" and \"!\""),
        "{hello}"
    );
    // A template literal is the same text.
    assert_eq!(
        ok_outline(&file("", "<Text>{`Hello, ${props.name}!`}</Text>")),
        hello
    );
    // A string expression is literal text; destructured props likewise.
    assert_eq!(
        ok_outline(&file("", "<Text>{\"Hello, \"}{props.name}!</Text>")),
        hello
    );
    let destructured = format!(
        "{KIT}export function S({{ name }}) {{ return <Screen><Column><Text>Hello, {{name}}!</Text></Column></Screen>; }}\n"
    );
    assert_eq!(ok_outline(&destructured), hello);
    // Literal text on one side only.
    let count = ok_outline(&file("", "<Text>{props.count} guests</Text>"));
    assert!(
        count.contains("slot 1 between \"\" and \" guests\""),
        "{count}"
    );
    // The prop alone is a slot as before; in a role, its literals too.
    let alone = ok_outline(&file("", "<Text>{props.name}</Text>"));
    assert!(alone.contains("slot 1\n"), "{alone}");
    let table = ok_outline(&file(
        "",
        "<Text role=\"display\">TABLE {props.table}</Text>",
    ));
    assert!(table.contains("between \"TABLE \" and \"\""), "{table}");
    let module = ok(&file("", "<Text>Hello, {props.name}!</Text>"));
    assert_eq!(module.screens[0].slots().len(), 1);
}

#[test]
fn in_a_part_a_literal_given_joins_the_text_and_a_prop_given_is_the_slot() {
    let part = "function Greeting({ name }) {\n  return <Text>Hello, {name}!</Text>;\n}";
    assert_eq!(
        ok_outline(&file(part, "<Greeting name={props.who} />")),
        ok_outline(&file("", "<Text>Hello, {props.who}!</Text>"))
    );
    assert_eq!(
        ok_outline(&file(part, "<Greeting name=\"Ada\" />")),
        ok_outline(&file("", "<Text>Hello, Ada!</Text>"))
    );
    // Omitted: nothing in JSX text, "undefined" in a template literal, as
    // React shows them.
    assert_eq!(
        ok_outline(&file(part, "<Greeting />")),
        ok_outline(&file("", "<Text>Hello, !</Text>"))
    );
    let template = "function Greeting({ name }) {\n  return <Text>{`Hello, ${name}!`}</Text>;\n}";
    assert_eq!(
        ok_outline(&file(template, "<Greeting />")),
        ok_outline(&file("", "<Text>Hello, undefined!</Text>"))
    );
}

#[test]
fn other_mixes_are_refused_where_written() {
    let e = error(&file("", "<Text>{props.a} of {props.b}</Text>"));
    assert_eq!(e.code, Code::Unsupported);
    assert!(e.message.contains("second prop"), "{}", e.message);
    let e = error(&file("", "<Text>{`${props.a} of ${props.b}`}</Text>"));
    assert!(e.message.contains("second prop"), "{}", e.message);
    // A call is reported, and the prop beside it is not.
    let e = error(&file("", "<Text>Hello, {props.name}{format()}</Text>"));
    assert!(e.message.contains("call to `format`"), "{}", e.message);
    let e = error(&file("", "<Text>{`Hello, ${format(props.name)}`}</Text>"));
    assert!(e.message.contains("call to `format`"), "{}", e.message);
    // Literals the role cannot draw, or too many of them.
    let e = error(&file(
        "",
        "<Text role=\"display\">Table {props.table}</Text>",
    ));
    assert_eq!(e.code, Code::Unsupported);
    let long = "x".repeat(65);
    let e = error(&file("", &format!("<Text>{long}{{props.name}}</Text>")));
    assert_eq!(e.code, Code::Limit);
    // A name that is not a prop, as before.
    let e = error(&file("", "<Text>Hello, {name}!</Text>"));
    assert_eq!(e.code, Code::UnknownName);
}

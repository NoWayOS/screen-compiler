// SPDX-License-Identifier: MIT
//! Lists, from `.map`: expanded at build time over a list the file holds,
//! or as rows of groups and slots the application fills over a screen's
//! prop with a most - each equal to the screen written out by hand.

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

const KIT: &str = "import { Button, Column, Row, Screen, Text } from \"nowayos:kit\";\n";

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
fn a_list_the_file_holds_is_expanded_at_build_time() {
    let flat = ok_outline(&file(
        "",
        "<Text>Tea</Text><Text>Coffee</Text><Text>Juice</Text>",
    ));
    assert_eq!(
        ok_outline(&file(
            "",
            "{[\"Tea\", \"Coffee\", \"Juice\"].map((drink) => <Text>{drink}</Text>)}"
        )),
        flat
    );
    assert_eq!(
        ok_outline(&file(
            "const DRINKS = [\"Tea\", \"Coffee\", \"Juice\"];",
            "{DRINKS.map((drink) => <Text key={drink}>{drink}</Text>)}"
        )),
        flat
    );
    // Objects, destructured or by field; `key` is React's alone.
    let objects = "const MENU = [{ id: 1, name: \"Tea\", price: \"£1.20\" }, { id: 2, name: \"Toast\", price: \"£2.00\" }];";
    let flat = ok_outline(&file(
        "",
        "<Row><Text>Tea</Text><Text>£1.20</Text></Row><Row><Text>Toast</Text><Text>£2.00</Text></Row>",
    ));
    assert_eq!(
        ok_outline(&file(
            objects,
            "{MENU.map(({ id, name, price }) => <Row key={id}><Text>{name}</Text><Text>{price}</Text></Row>)}"
        )),
        flat
    );
    assert_eq!(
        ok_outline(&file(
            objects,
            "{MENU.map((dish) => { return <Row key={dish.id}><Text>{dish.name}</Text><Text>{dish.price}</Text></Row>; })}"
        )),
        flat
    );
    // Sliced: the first ones.
    assert_eq!(
        ok_outline(&file(
            objects,
            "{MENU.slice(0, 1).map((dish) => <Text>{dish.name}</Text>)}"
        )),
        ok_outline(&file("", "<Text>Tea</Text>")),
    );
}

#[test]
fn a_prop_list_with_a_most_is_rows_of_groups_and_slots_the_application_fills() {
    let rows = "{props.items.slice(0, 3).map((item) => <Row key={item.id}><Text>{item.name}</Text><Text>{item.price}</Text></Row>)}";
    let by_hand: String = (1..=3)
        .map(|r| {
            format!(
                "{{props.items_{r} && <Row><Text>{{props.items_{r}_name}}</Text><Text>{{props.items_{r}_price}}</Text></Row>}}"
            )
        })
        .collect();
    assert_eq!(ok_outline(&file("", rows)), ok_outline(&file("", &by_hand)));
    let module = ok(&file("", rows));
    let screen = &module.screens[0];
    assert_eq!(screen.groups().len(), 3, "one group per row");
    assert_eq!(screen.slots().len(), 6, "a slot per field per row");
    // Destructured, in a screen whose props are destructured; an item
    // itself is the slot `…_item`.
    let source = format!(
        "{KIT}export function S({{ names }}) {{ return <Screen><Column>{{names.slice(0, 2).map((name) => <Text>{{name}}</Text>)}}</Column></Screen>; }}\n"
    );
    let by_hand = file(
        "",
        "{props.names_1 && <Text>{props.names_1_item}</Text>}{props.names_2 && <Text>{props.names_2_item}</Text>}",
    );
    assert_eq!(ok_outline(&source), ok_outline(&by_hand));
}

#[test]
fn a_list_inside_a_part_sees_the_part_s_props_and_its_own_item() {
    let parts = "function Menu({ heading, dishes }) {\n  return <Column><Text role=\"heading\">{heading}</Text>{dishes.map((dish) => <Text>{dish}</Text>)}</Column>;\n}";
    assert_eq!(
        ok_outline(&file(
            parts,
            "<Menu heading=\"Today\" dishes={[\"Soup\", \"Bread\"]} />"
        )),
        ok_outline(&file(
            "",
            "<Column><Text role=\"heading\">Today</Text><Text>Soup</Text><Text>Bread</Text></Column>"
        )),
    );
}

#[test]
fn a_list_with_no_most_and_other_lists_are_refused_where_written() {
    let e = error(&file(
        "",
        "{props.items.map((item) => <Text>{item}</Text>)}",
    ));
    assert_eq!(e.code, Code::Unsupported);
    assert!(e.message.contains("has no most"), "{}", e.message);
    for most in ["0", "33", "props.n"] {
        let e = error(&file(
            "",
            &format!("{{props.items.slice(0, {most}).map((item) => <Text>{{item}}</Text>)}}"),
        ));
        assert_eq!(e.code, Code::InvalidValue, "{most}");
    }
    let e = error(&file(
        "",
        "{props.items.slice(0, 2).map((item) => <><Text>{item}</Text><Text>x</Text></>)}",
    ));
    assert!(e.message.contains("must be one element"), "{}", e.message);
    let e = error(&file(
        "",
        "{props.rows.slice(0, 2).map((row) => <Column>{props.cells.slice(0, 2).map((c) => <Text>{c}</Text>)}</Column>)}",
    ));
    assert!(
        e.message.contains("inside a row of another"),
        "{}",
        e.message
    );
    let e = error(&file("", "{[\"a\"].map((x, i) => <Text>{x}</Text>)}"));
    assert!(e.message.contains("takes the item alone"), "{}", e.message);
    let e = error(&file(
        "",
        "{[\"a\"].map((x) => { const y = x; return <Text>{y}</Text>; })}",
    ));
    assert!(e.message.contains("only returns"), "{}", e.message);
    let e = error(&file(
        "",
        "{elsewhere.slice(0, 2).map((x) => <Text>{x}</Text>)}",
    ));
    assert!(e.message.contains("cannot be listed"), "{}", e.message);
}

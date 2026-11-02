// SPDX-License-Identifier: MIT
//! Fragments and a screen's own components ("parts"), expanded at build
//! time: a screen built of parts is the screen written flat, and what React
//! would not run, or the subset cannot keep exact, is refused where written.

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

const KIT: &str = "import { Button, Card, Checkbox, Column, HeaderBar, Notice, Row, Screen, Spacer, Text, TextInput, space } from \"nowayos:kit\";\n";

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

/// The outline of the file's only screen.
fn ok_outline(source: &str) -> String {
    let module = ok(source);
    assert_eq!(module.screens.len(), 1, "{source}");
    outline(&module, &module.screens[0])
}

fn errors(source: &str) -> Vec<Diagnostic> {
    let analysis = run(source);
    assert!(
        analysis.module.is_none(),
        "an error must stop conversion:\n{source}"
    );
    analysis
        .diagnostics
        .into_iter()
        .filter(Diagnostic::is_error)
        .collect()
}

/// The one error `source` gives.
fn error(source: &str) -> Diagnostic {
    let all = errors(source);
    assert_eq!(all.len(), 1, "{all:#?}\n{source}");
    all.into_iter().next().unwrap()
}

/// A file of `parts` and one screen whose `<Column>` holds `body`.
fn file(parts: &str, body: &str) -> String {
    format!(
        "{KIT}const GAP = 12;\n{parts}\nexport function S(props) {{ return <Screen><Column gap={{GAP}}>{body}</Column></Screen>; }}\n"
    )
}

#[test]
fn a_screen_built_of_parts_is_the_screen_written_flat() {
    let parts = "\
function Header({ title, onHome }) {
  return (
    <HeaderBar gap={GAP} padding={8}>
      <Text role=\"heading\">{title}</Text>
      <Spacer />
      <Button variant=\"secondary\" onPress={onHome}>Home</Button>
    </HeaderBar>
  );
}
const Panel = (props) => <Card gap={props.gap} padding={space.md}>{props.children}</Card>;
function Answer({ label, field }) {
  return <><Text role=\"label\">{label}</Text><TextInput id={field} maxLength={20} /></>;
}";
    let composed = file(
        parts,
        "<Header title=\"Visitor check-in\" onHome={props.onHome} />\
         <Panel gap={4}><Answer label=\"Your name\" field=\"name\" /><Answer label=\"Your host\" field=\"host\" /></Panel>\
         {props.late && <Panel gap={GAP}><Text>{props.lateNotice}</Text></Panel>}",
    );
    let flat = file(
        "",
        "<HeaderBar gap={GAP} padding={8}><Text role=\"heading\">Visitor check-in</Text><Spacer />\
         <Button variant=\"secondary\" onPress={props.onHome}>Home</Button></HeaderBar>\
         <Card gap={4} padding={space.md}><Text role=\"label\">Your name</Text><TextInput id=\"name\" maxLength={20} />\
         <Text role=\"label\">Your host</Text><TextInput id=\"host\" maxLength={20} /></Card>\
         {props.late && <Card gap={GAP} padding={space.md}><Text>{props.lateNotice}</Text></Card>}",
    );
    assert_eq!(ok_outline(&composed), ok_outline(&flat));
}

#[test]
fn fragments_take_their_place_wherever_children_are_taken() {
    let flat = ok_outline(&file("", "<Text>a</Text><Text>b</Text>"));
    assert_eq!(
        ok_outline(&file("", "<><Text>a</Text><Text>b</Text></>")),
        flat
    );
    assert_eq!(
        ok_outline(&file("", "<><><Text>a</Text></><Text>b</Text></>")),
        flat
    );
    // A fragment of one element is that element, as a condition shows it;
    // and text in a fragment is a text's.
    assert_eq!(
        ok_outline(&file("", "{props.ready && <><Text>a</Text></>}")),
        ok_outline(&file("", "{props.ready && <Text>a</Text>}")),
    );
    assert_eq!(
        ok_outline(&file("", "<Text><>Hello</></Text>")),
        ok_outline(&file("", "<Text>Hello</Text>")),
    );
}

#[test]
fn a_prop_a_use_does_not_give_is_undefined_as_in_react() {
    let parts = "function Go({ label, kind, onGo, show }) {\n  return <>{show && <Text>{label}</Text>}<Button variant={kind} onPress={onGo}>Go</Button></>;\n}";
    // No `kind`: the button keeps its default variant; no `show`: the
    // condition shows nothing.
    assert_eq!(
        ok_outline(&file(parts, "<Go label=\"x\" onGo={props.onGo} />")),
        ok_outline(&file("", "<Button onPress={props.onGo}>Go</Button>")),
    );
    // No `onGo`: the button inside the part has no action, reported there.
    let e = error(&file(parts, "<Go label=\"x\" />"));
    assert_eq!((e.code, e.loc.unwrap().line), (Code::MissingProp, 4));
}

#[test]
fn a_part_may_use_parts_and_pass_its_props_and_children_on() {
    let parts = "\
function Outer({ title, children }) { return <Card padding={8}><Inner heading={title}>{children}</Inner></Card>; }
function Inner({ heading, children }) { return <Column gap={4}><Text role=\"heading\">{heading}</Text>{children}</Column>; }";
    assert_eq!(
        ok_outline(&file(parts, "<Outer title=\"T\"><Text>a</Text></Outer>")),
        ok_outline(&file(
            "",
            "<Card padding={8}><Column gap={4}><Text role=\"heading\">T</Text><Text>a</Text></Column></Card>"
        )),
    );
    // A part used inside its own children is fine: React renders them.
    assert_eq!(
        ok_outline(&file(
            parts,
            "<Outer title=\"A\"><Outer title=\"B\" /></Outer>"
        )),
        ok_outline(&file(
            "",
            "<Card padding={8}><Column gap={4}><Text role=\"heading\">A</Text>\
             <Card padding={8}><Column gap={4}><Text role=\"heading\">B</Text></Column></Card></Column></Card>"
        )),
    );
}

#[test]
fn a_screen_may_be_a_part_at_its_root() {
    let parts = "function Page({ title, children }) { return <Screen><Column><Text role=\"title\">{title}</Text>{children}</Column></Screen>; }";
    let source = format!(
        "{KIT}{parts}\nexport function S() {{ return <Page title=\"Hi\"><Text>a</Text></Page>; }}\n"
    );
    let flat = format!(
        "{KIT}export function S() {{ return <Screen><Column><Text role=\"title\">Hi</Text><Text>a</Text></Column></Screen>; }}\n"
    );
    assert_eq!(ok_outline(&source), ok_outline(&flat));
}

#[test]
fn numbers_stay_one_per_prop_and_ids_one_per_field() {
    let parts = "function Field({ field, label }) { return <><Text>{label}</Text><TextInput id={field} /></>; }\nfunction Save({ onSave }) { return <Button onPress={onSave}>Save</Button>; }";
    let module = ok(&file(
        parts,
        "<Field field=\"a\" label=\"A\" /><Field field=\"b\" label=\"B\" /><Save onSave={props.onSave} /><Save onSave={props.onSave} />",
    ));
    let screen = &module.screens[0];
    assert_eq!(
        screen.actions().len(),
        1,
        "one action per prop, as written flat"
    );
    // The same id twice is two fields of one id: refused, as flat.
    let all = errors(&file(
        parts,
        "<Field field=\"a\" label=\"A\" /><Field field=\"a\" label=\"B\" />",
    ));
    assert!(
        all.iter().any(|d| d.code == Code::DuplicateName),
        "{all:#?}"
    );
}

#[test]
fn recursion_is_refused_where_the_part_uses_itself() {
    let e = error(&file(
        "function A() { return <Column><A /></Column>; }",
        "<A />",
    ));
    assert_eq!(e.code, Code::Unsupported);
    assert!(e.message.contains("A > A"), "{}", e.message);
    let all = errors(&file(
        "function A() { return <B />; }\nfunction B() { return <Column><A /></Column>; }",
        "<A />",
    ));
    assert!(
        all.iter().any(|d| d.message.contains("A > B > A")),
        "{all:#?}"
    );
}

#[test]
fn spreads_the_whole_props_object_and_children_as_a_prop_are_refused() {
    let part = "function P({ title }) { return <Text>{title}</Text>; }";
    let e = error(&file(
        "function P() { return <Text>x</Text>; }",
        "<P {...props} />",
    ));
    assert_eq!((e.code, e.loc.unwrap().line), (Code::Unsupported, 4));
    let e = error(&file(
        "function P(props) { return <Q all={props} />; }\nfunction Q({ all }) { return <Text>x</Text>; }",
        "<P />",
    ));
    assert!(e.message.contains("whole props object"), "{}", e.message);
    let e = error(&file(part, "<P title=\"x\" children={props.a} />"));
    assert!(e.message.contains("`children` as a prop"), "{}", e.message);
}

#[test]
fn a_part_sees_only_its_props_and_the_file_s_names() {
    // `onGo` is the screen's prop, not the part's: React would not give it.
    let e = error(
        &file(
            "function P() { return <Button onPress={onGo}>Go</Button>; }",
            "<P />",
        )
        .replace("S(props)", "S({ onGo })"),
    );
    assert_eq!(e.code, Code::UnknownName);
    assert!(e.message.contains("not a prop of <P>"), "{}", e.message);
    // The file's own design tokens and the kit's spacing tokens it may use.
    ok(&file(
        "function P() { return <Column gap={GAP} padding={space.sm} />; }",
        "<P />",
    ));
}

#[test]
fn a_prop_given_no_value_is_true_and_refused_where_used_as_a_value() {
    let e = error(&file(
        "function P({ shown }) { return <>{shown && <Text>x</Text>}</>; }",
        "<P shown />",
    ));
    assert_eq!(e.code, Code::Unsupported);
    assert!(e.message.contains("given no value"), "{}", e.message);
}

#[test]
fn parts_nest_sixteen_deep_and_no_deeper() {
    // P0 to P`top`, each using the one below: `top + 1` parts deep.
    let chain = |top: usize| {
        let mut parts = String::from("function P0() { return <Text>x</Text>; }\n");
        for n in 1..=top {
            parts.push_str(&format!("function P{n}() {{ return <P{} />; }}\n", n - 1));
        }
        file(&parts, &format!("<P{top} />"))
    };
    ok(&chain(15));
    let all = errors(&chain(16));
    assert!(all.iter().any(|d| d.code == Code::Limit), "{all:#?}");
}

#[test]
fn a_screen_that_would_expand_too_far_is_refused_without_building_it() {
    // Each part uses the one below twice: 2^20 elements, written in a few lines.
    let mut parts = String::from("function P0() { return <Text>x</Text>; }\n");
    for n in 1..=15 {
        parts.push_str(&format!(
            "function P{n}() {{ return <><P{m} /><P{m} /></>; }}\n",
            m = n - 1
        ));
    }
    let all = errors(&file(&parts, "<P15 />"));
    assert!(all.iter().any(|d| d.code == Code::Limit), "{all:#?}");
}

#[test]
fn an_error_inside_a_part_used_twice_is_reported_once() {
    let all = errors(&file(
        "function P() { return <Button>Go</Button>; }",
        "<P /><P />",
    ));
    assert_eq!(all.len(), 1, "{all:#?}");
}

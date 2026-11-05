// SPDX-License-Identifier: MIT
//! Subset v0, rule by rule: what converts, and what is reported where.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use screen_compiler_analyse::{Analysis, Limits, Options, analyse};
use screen_compiler_ir::{Code, Diagnostic, Loc, Module, outline};

/// Line 1 of most test sources.
const KIT: &str = "import { Button, Card, Checkbox, Column, HeaderBar, Icon, Notice, ProgressBar, RadioGroup, Row, Screen, Spacer, Text, TextInput } from \"nowayos:kit\";\n";

fn run_with(source: &str, options: &Options) -> Analysis {
    match screen_compiler_tsx::parse(source) {
        Ok(parsed) => analyse(&parsed, options),
        Err(diagnostics) => Analysis {
            module: None,
            diagnostics,
            imported: Vec::new(),
        },
    }
}

fn run(source: &str) -> Analysis {
    run_with(source, &Options::new("test.tsx"))
}

/// The module, or a panic listing the diagnostics.
fn ok(source: &str) -> Module {
    let analysis = run(source);
    match analysis.module {
        Some(module) => module,
        None => panic!("expected a module, got {:#?}", analysis.diagnostics),
    }
}

/// The outline of the only screen.
fn ok_outline(source: &str) -> String {
    let module = ok(source);
    assert_eq!(module.screens.len(), 1);
    outline(&module, &module.screens[0])
}

/// Every error, and a check that no IR was produced.
fn errors(source: &str) -> Vec<Diagnostic> {
    let analysis = run(source);
    assert!(analysis.module.is_none(), "an error must stop conversion");
    let errors: Vec<Diagnostic> = analysis
        .diagnostics
        .into_iter()
        .filter(Diagnostic::is_error)
        .collect();
    assert!(!errors.is_empty(), "expected errors");
    errors
}

/// The single error.
fn error(source: &str) -> Diagnostic {
    let mut all = errors(source);
    assert_eq!(all.len(), 1, "expected one error, got {all:#?}");
    all.remove(0)
}

fn at(line: u32, column: u32) -> Option<Loc> {
    Some(Loc::new(line, column))
}

/// `KIT` plus a component returning `<Screen>{body}</Screen>`, on line 2.
fn screen(body: &str) -> String {
    format!("{KIT}export function S({{ onGo, onBack }}: P) {{ return <Screen>{body}</Screen>; }}\n")
}

#[test]
fn a_minimal_screen_converts() {
    assert_eq!(ok_outline(&screen("<Text>Hi</Text>")), "text \"Hi\"\n");
}

#[test]
fn a_full_screen_converts_with_tokens_ids_and_actions() {
    let source = format!(
        "{KIT}const GAP = 8;\nexport const Form = ({{ onGo, onBack: back }}: P) => (\n  <Screen>\n    <Column gap={{GAP}} padding={{4}} id=\"main\">\n      <TextInput id=\"email\" placeholder=\"Email\" />\n      <Row>\n        <Button onPress={{back}}>Back</Button>\n        <Spacer />\n        <Button onPress={{onGo}}>Go</Button>\n        <Button onPress={{back}}>Back again</Button>\n      </Row>\n      <Spacer size={{GAP}} />\n    </Column>\n  </Screen>\n);\n"
    );
    let module = ok(&source);
    assert_eq!(module.tokens.len(), 1);
    let screen = &module.screens[0];
    assert_eq!(screen.name.source(), "Form");
    assert_eq!(
        outline(&module, screen),
        "\
column gap=8 padding=4
  input field=1 max_len=64 mode=text placeholder=\"Email\"
  row gap=0 padding=0
    button \"Back\" action=1
    spacer
    button \"Go\" action=2
    button \"Back again\" action=1
  spacer size=8
"
    );
    let actions: Vec<&str> = screen.actions().iter().map(|a| a.source()).collect();
    // Actions are named after the prop, not the local alias.
    assert_eq!(actions, ["onBack", "onGo"]);
    assert_eq!(screen.actions()[0].words(), ["back"]);
    assert_eq!(screen.actions()[1].words(), ["go"]);
    let ids: Vec<&str> = screen
        .nodes()
        .iter()
        .filter_map(|n| n.id.as_ref().map(|id| id.source()))
        .collect();
    assert_eq!(ids, ["main", "email"]);
}

#[test]
fn the_props_object_form_works() {
    let source = format!(
        "{KIT}export default function S(props: P) {{\n  return <Screen><Button onPress={{props.onNext}}>Next</Button></Screen>;\n}}\n"
    );
    assert_eq!(ok_outline(&source), "button \"Next\" action=1\n");
}

#[test]
fn aliased_kit_imports_resolve() {
    let source = "import { Screen, Column as Stack, Text as Label } from \"nowayos:kit\";\nexport function S() { return <Screen><Stack><Label>x</Label></Stack></Screen>; }\n";
    assert_eq!(ok_outline(source), "column gap=0 padding=0\n  text \"x\"\n");
}

#[test]
fn jsx_text_rules_and_string_literals_reach_the_ir() {
    let source = screen("<Text>\n  Terms &amp; conditions\n  apply.\n</Text>");
    assert_eq!(ok_outline(&source), "text \"Terms & conditions apply.\"\n");
    let source = screen("<Text>Hello{\" \"}world{/* note */}</Text>");
    assert_eq!(ok_outline(&source), "text \"Hello world\"\n");
    // React drops the space between text and an expression on separate lines.
    let source = screen("<Text>\n  Hello\n  {\"world\"}\n</Text>");
    assert_eq!(ok_outline(&source), "text \"Helloworld\"\n");
}

#[test]
fn react_default_imports_and_types_are_ignored() {
    let source = format!(
        "import React from \"react\";\nimport type {{ ReactNode }} from \"react\";\nimport type {{ Theme }} from \"./theme\";\n{KIT}type P = {{ a: string }};\ninterface Q {{ b: number }}\nexport function S() {{ return <Screen><Text>x</Text></Screen>; }}\nexport default S;\n"
    );
    ok(&source);
}

#[test]
fn design_tokens_must_be_const_whole_numbers() {
    let e = error(&format!(
        "{KIT}const HALF = 1.5;\n{}",
        screen("<Text>x</Text>").trim_start_matches(KIT)
    ));
    assert_eq!((e.code, e.loc), (Code::InvalidValue, at(2, 14)));
    let e = error(&format!(
        "{KIT}let GAP = 8;\n{}",
        screen("<Text>x</Text>").trim_start_matches(KIT)
    ));
    assert_eq!((e.code, e.loc), (Code::Unsupported, at(2, 5)));
    let e = error(&format!(
        "{KIT}const T = {{ md: 16 }};\n{}",
        screen("<Text>x</Text>").trim_start_matches(KIT)
    ));
    assert_eq!(e.code, Code::Unsupported);
    assert!(e.message.contains("an object literal"), "{}", e.message);
    let e = error(&format!(
        "{KIT}const BIG = 70000;\n{}",
        screen("<Text>x</Text>").trim_start_matches(KIT)
    ));
    assert_eq!(e.code, Code::InvalidValue);
}

#[test]
fn lengths_are_pixels_or_tokens() {
    // A length in px is that many layout units (tests/css_units.rs);
    // other units are refused where written.
    let e = error(&screen("<Column gap=\"1rem\" />"));
    assert_eq!((e.code, e.loc), (Code::Unsupported, at(2, 69)));
    assert!(e.help.unwrap().contains("16px"));
    let e = error(&screen("<Column gap={NOPE} />"));
    assert_eq!((e.code, e.loc), (Code::UnknownName, at(2, 70)));
    let e = error(&screen("<Column gap={onGo} />"));
    assert_eq!(e.code, Code::Unsupported);
    let e = error(&screen("<Column gap={-4} />"));
    assert_eq!(e.code, Code::Unsupported);
    let e = error(&screen("<Column gap={theme.space} />"));
    assert_eq!(e.code, Code::Unsupported);
    let e = error(&screen("<Column gap />"));
    assert_eq!(e.code, Code::InvalidValue);
}

#[test]
fn buttons_need_a_label_and_a_callback_prop() {
    let e = error(&screen("<Button>Go</Button>"));
    assert_eq!((e.code, e.loc), (Code::MissingProp, at(2, 57)));
    let e = error(&screen("<Button onPress={onGo}></Button>"));
    assert_eq!(e.code, Code::InvalidChildren);
    let e = error(&screen("<Button onPress={nothing}>Go</Button>"));
    assert_eq!(e.code, Code::UnknownName);
    let e = error(&screen("<Button onPress=\"go\">Go</Button>"));
    assert_eq!(e.code, Code::InvalidValue);
}

#[test]
fn a_button_takes_react_doms_on_click_and_a_function_that_only_calls_a_prop() {
    let pressed = ok_outline(&screen("<Button onPress={onGo}>Go</Button>"));
    for written in [
        "<Button onClick={onGo}>Go</Button>",
        "<Button onPress={() => onGo()}>Go</Button>",
        "<Button onClick={() => onGo()}>Go</Button>",
        "<Button onClick={() => { onGo(); }}>Go</Button>",
        "<Button onClick={function () { onGo(); }}>Go</Button>",
    ] {
        assert_eq!(ok_outline(&screen(written)), pressed, "{written}");
    }
    let by_object = format!(
        "{KIT}export function S(props: P) {{ return <Screen><Button onClick={{() => props.onGo()}}>Go</Button></Screen>; }}\n"
    );
    assert_eq!(ok_outline(&by_object), pressed);
    // One of the two, never both.
    let e = error(&screen(
        "<Button onPress={onGo} onClick={onBack}>Go</Button>",
    ));
    assert_eq!((e.code, e.loc), (Code::DuplicateName, at(2, 80)));
    assert!(e.message.contains("both onPress and onClick"), "{e:#?}");
    // A function that does anything more is refused where it starts.
    for written in [
        "<Button onClick={() => { onGo(); onBack(); }}>Go</Button>",
        "<Button onClick={(event) => onGo(event)}>Go</Button>",
        "<Button onClick={() => onGo(1)}>Go</Button>",
        "<Button onClick={async () => onGo()}>Go</Button>",
        "<Button onClick={() => onGo?.()}>Go</Button>",
        "<Button onClick={() => go.now.later()}>Go</Button>",
    ] {
        let e = error(&screen(written));
        assert_eq!(
            (e.code, e.loc),
            (Code::Unsupported, at(2, 74)),
            "{written}: {e:#?}"
        );
    }
    // What it calls must be a callback prop.
    let e = error(&screen("<Button onClick={() => go()}>Go</Button>"));
    assert_eq!(e.code, Code::UnknownName);
}

#[test]
fn text_inputs_need_an_id() {
    let e = error(&screen("<TextInput placeholder=\"Name\" />"));
    assert_eq!(e.code, Code::MissingProp);
    ok(&screen("<TextInput id=\"name\" />"));
    let e = error(&screen("<TextInput id=\"n\" value=\"x\" />"));
    assert_eq!(e.code, Code::UnknownProp);
    assert!(e.help.unwrap().contains("state"));
}

#[test]
fn ids_are_validated_and_unique() {
    let e = error(&screen("<TextInput id=\"1st\" />"));
    assert_eq!(e.code, Code::InvalidValue);
    let e = error(&screen("<TextInput id={1} />"));
    assert_eq!(e.code, Code::InvalidValue);
    let e = error(&screen(
        "<Column><TextInput id=\"fullName\" /><TextInput id=\"full-name\" /></Column>",
    ));
    assert_eq!(e.code, Code::DuplicateName);
    ok(&screen(
        "<Column><TextInput id=\"a\" /><TextInput id={\"b\"} /></Column>",
    ));
}

#[test]
fn screen_holds_exactly_one_element_and_only_at_the_root() {
    let e = error(&screen("<Text>a</Text><Text>b</Text>"));
    assert_eq!((e.code, e.loc), (Code::InvalidChildren, at(2, 71)));
    let e = error(&screen(""));
    assert_eq!(e.code, Code::InvalidChildren);
    let e = error(&screen("<Column><Screen /></Column>"));
    assert_eq!(e.code, Code::InvalidChildren);
    let e = error(&format!(
        "{KIT}export function S() {{ return <Screen id=\"x\"><Text>a</Text></Screen>; }}\n"
    ));
    assert_eq!(e.code, Code::UnknownProp);
}

#[test]
fn text_is_literal_and_lives_in_text() {
    let e = error(&screen("<Column>loose text</Column>"));
    assert_eq!((e.code, e.loc), (Code::InvalidChildren, at(2, 65)));
    // A prop alone is a slot (subset v1), or between literal text
    // (contract v14, tests/text.rs); a second prop is refused where written.
    let e = error(&screen("<Text>Hi {onGo} and {onBack}</Text>"));
    assert_eq!((e.code, e.loc), (Code::Unsupported, at(2, 78)));
    assert!(e.help.as_ref().unwrap().contains("give each prop its text"));
    let e = error(&screen("<Text>{elsewhere}</Text>"));
    assert_eq!((e.code, e.loc), (Code::UnknownName, at(2, 64)));
    let e = error(&screen("<Text>Hi <Button onPress={onGo}>x</Button></Text>"));
    assert_eq!(e.code, Code::InvalidChildren);
    let e = error(&screen("<Column>{\"loose\"}</Column>"));
    assert_eq!(e.code, Code::InvalidChildren);
}

#[test]
fn html_and_custom_elements_are_reported_with_their_contents() {
    let all = errors(&screen("<div className=\"x\"><TextInput /></div>"));
    let codes: Vec<Code> = all.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::UnknownElement, Code::MissingProp]);
    // A component the file uses as an element is a part, expanded where
    // it is used (tests/parts.rs).
    let source = format!(
        "{KIT}function Header() {{ return <Row />; }}\nexport function S() {{ return <Screen><Header /></Screen>; }}\n"
    );
    assert_eq!(ok_outline(&source), "row gap=0 padding=0\n");
    let source = "import { Screen } from \"nowayos:kit\";\nexport function S() { return <Screen><Column /></Screen>; }\n";
    let e = error(source);
    assert_eq!(e.code, Code::UnknownElement);
    assert!(e.message.contains("not imported"));
}

#[test]
fn lists_conditionals_fragments_and_spreads_are_reported() {
    let codes =
        |body: &str| -> Vec<Code> { errors(&screen(body)).iter().map(|d| d.code).collect() };
    assert_eq!(
        codes("<Column>{items.map((i) => <Text>{i}</Text>)}</Column>"),
        [Code::Unsupported]
    );
    // Conditions are one prop (subset v1), negated or compared with a
    // string literal (contract v13); anything else is reported.
    assert_eq!(
        codes("<Column>{elsewhere && <Text>x</Text>}</Column>"),
        [Code::UnknownName]
    );
    assert_eq!(
        codes("<Column>{onGo || <Text>x</Text>}</Column>"),
        [Code::Unsupported]
    );
    assert_eq!(
        codes("<Column>{onGo === \"a\" ? null : <Text>a</Text>}</Column>"),
        [Code::Unsupported]
    );
    // A fragment's children take its place: two where <Screen> takes one.
    assert_eq!(
        codes("<><Text>a</Text><Text>b</Text></>"),
        [Code::InvalidChildren]
    );
    assert_eq!(
        codes("<Column><><Text>{x}</Text></></Column>"),
        [Code::UnknownName]
    );
    assert_eq!(codes("<Column {...rest} />"), [Code::Unsupported]);
    assert_eq!(codes("<Column gap={1} gap={2} />"), [Code::DuplicateName]);
}

/// `KIT` plus a component taking `props`, returning `<Screen>{body}</Screen>`
/// on line 2.
fn props_screen(body: &str) -> String {
    format!("{KIT}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n")
}

/// The column, on line 2 of [`props_screen`], of `marker`'s first
/// occurrence in `body` (all ASCII).
fn column_of(body: &str, marker: &str) -> u32 {
    let prefix = "export function S(props: P) { return <Screen>".len();
    u32::try_from(prefix + body.find(marker).unwrap() + 1).unwrap()
}

#[test]
fn a_prop_alone_in_text_is_a_slot_the_application_fills() {
    // Subset v1: never text compiled in from the source.
    assert_eq!(
        ok_outline(&props_screen(
            "<Row><Text>Name:</Text><Text id=\"name\">{props.name}</Text><Text>{props.when}</Text></Row>"
        )),
        "row gap=0 padding=0\n  text \"Name:\"\n  slot 1\n  slot 2\n"
    );
    // Destructured, with JSX comments beside it.
    assert_eq!(
        ok_outline(&screen("<Text>{/* the code */}{onGo}</Text>")),
        "slot 1\n"
    );
    let module = ok(&props_screen("<Text>{props.lateNotice}</Text>"));
    let slots = module.screens[0].slots();
    assert_eq!(slots[0].source(), "lateNotice");
    // Mixed with text, or beside spaces React keeps, it is between
    // literal text (contract v14); a template literal of the prop alone is
    // the prop.
    for (body, outline) in [
        (
            "<Text>Tries left: {props.left}</Text>",
            "slot 1 between \"Tries left: \" and \"\"\n",
        ),
        (
            "<Text> {props.left}</Text>",
            "slot 1 between \" \" and \"\"\n",
        ),
        ("<Text>{`${props.left}`}</Text>", "slot 1\n"),
    ] {
        assert_eq!(ok_outline(&props_screen(body)), outline, "{body}");
    }
    // Twice, or with anything else: refused where written.
    for (body, code, marker) in [
        (
            "<Text>{props.left}{props.right}</Text>",
            Code::Unsupported,
            "props.right",
        ),
        ("<Text>{props}</Text>", Code::InvalidValue, "props}"),
        ("<Text>{other.left}</Text>", Code::UnknownName, "other"),
        (
            "<Text>{props.a + props.b}</Text>",
            Code::Unsupported,
            "props.a",
        ),
        (
            "<Text>{`${props.a()}`}</Text>",
            Code::Unsupported,
            "props.a",
        ),
    ] {
        let all = errors(&props_screen(body));
        let loc = at(2, column_of(body, marker));
        assert!(
            all.iter().any(|e| e.code == code && e.loc == loc),
            "{body}: expected {code:?} at {loc:?}, got {all:#?}"
        );
    }
    let all = errors(&props_screen(
        "<Column><Text>{props.left}</Text><Text>{props.left}</Text></Column>",
    ));
    assert_eq!(all.len(), 1);
    assert_eq!((all[0].code, all[0].loc), (Code::DuplicateName, at(2, 79)));
    // A button's label stays literal.
    let e = error(&screen("<Button onPress={onGo}>{onGo}</Button>"));
    assert_eq!(e.code, Code::Unsupported);
    assert!(e.help.as_ref().unwrap().contains("literal"));
}

#[test]
fn a_prop_shows_or_hides_one_element_or_chooses_between_two() {
    // Subset v1: a group named after the prop, hidden at first.
    let outline = ok_outline(&props_screen(
        "<Column>{props.ready && <Text>Ready</Text>}{props.late ? <Text>Late</Text> : <Text>On time</Text>}{props.ready ? <Spacer /> : null}</Column>",
    ));
    assert_eq!(
        outline,
        "column gap=0 padding=0\n  group 1\n    text \"Ready\"\n  group 2\n    text \"Late\"\n    text \"On time\"\n  group 1\n    spacer\n"
    );
    // Destructured props, a group holding a slot, and parentheses.
    assert_eq!(
        ok_outline(&screen(
            "<Column>{onGo && (<Text>{onBack}</Text>)}</Column>"
        )),
        "column gap=0 padding=0\n  group 1\n    slot 1\n"
    );
    // Each refused where it is written: at the marker's first character.
    for (body, code, marker) in [
        // conditions other than one prop
        (
            "<Column>{props.n > 0 && <Text>x</Text>}</Column>",
            Code::Unsupported,
            "props.n",
        ),
        (
            "<Column>{!!props.ok && <Text>x</Text>}</Column>",
            Code::Unsupported,
            "!!props",
        ),
        (
            "<Column>{props.a === props.b && <Text>x</Text>}</Column>",
            Code::Unsupported,
            "props.a",
        ),
        (
            "<Column>{props.n === 1 && <Text>x</Text>}</Column>",
            Code::Unsupported,
            "props.n",
        ),
        (
            "<Column>{props.a && props.b && <Text>x</Text>}</Column>",
            Code::Unsupported,
            "props.a",
        ),
        (
            "<Column>{isReady() ? <Text>a</Text> : null}</Column>",
            Code::Unsupported,
            "isReady",
        ),
        (
            "<Column>{props && <Text>x</Text>}</Column>",
            Code::InvalidValue,
            "props &&",
        ),
        (
            "<Column>{ready && <Text>x</Text>}</Column>",
            Code::UnknownName,
            "ready",
        ),
        // operators other than && and ?:
        (
            "<Column>{props.a || <Text>x</Text>}</Column>",
            Code::Unsupported,
            "props.a",
        ),
        (
            "<Column>{props.a ?? <Text>x</Text>}</Column>",
            Code::Unsupported,
            "props.a",
        ),
        // what a condition shows: one element
        (
            "<Column>{props.a && \"text\"}</Column>",
            Code::InvalidChildren,
            "\"text",
        ),
        (
            "<Column>{props.a && <><Text>x</Text><Text>y</Text></>}</Column>",
            Code::Unsupported,
            "<>",
        ),
        (
            "<Column>{props.a && (props.b && <Text>x</Text>)}</Column>",
            Code::Unsupported,
            "props.b",
        ),
        (
            "<Column>{props.a === \"b\" ? null : <Text>x</Text>}</Column>",
            Code::Unsupported,
            "null",
        ),
        (
            "<Column>{props.a ? <Text>x</Text> : props.b}</Column>",
            Code::Unsupported,
            "props.b",
        ),
        // a screen's one child is an element
        ("{props.a && <Column />}", Code::InvalidChildren, "props.a"),
    ] {
        let all = errors(&props_screen(body));
        let loc = at(2, column_of(body, marker));
        assert!(
            all.iter().any(|e| e.code == code && e.loc == loc),
            "{body}: expected {code:?} at {loc:?}, got {all:#?}"
        );
    }
    // What a condition holds is checked too.
    let body = "<Column>{props.a && <div />}</Column>";
    let e = error(&props_screen(body));
    assert_eq!(
        (e.code, e.loc),
        (Code::UnknownElement, at(2, column_of(body, "<div")))
    );
}

#[test]
fn form_controls_are_fields_numbered_in_document_order() {
    // Subset v1.
    let source = format!(
        "{KIT}const LEN = 12;\nexport function S() {{ return <Screen><Column>\
         <TextInput id=\"name\" maxLength={{LEN}} />\
         <RadioGroup id=\"when\" options={{[\"Morning\", 'Evening']}} />\
         <TextInput id=\"pin\" placeholder=\"PIN\" inputMode=\"numeric\" maxLength={{4}} />\
         <Checkbox id=\"agree\" label=\"I agree\" />\
         <TextInput id=\"mail\" inputMode=\"email\" />\
         <TextInput id=\"tel\" inputMode=\"tel\" />\
         <TextInput id=\"any\" inputMode=\"text\" maxLength={{64}} />\
         </Column></Screen>; }}\n"
    );
    assert_eq!(
        ok_outline(&source),
        "column gap=0 padding=0\n  input field=1 max_len=12 mode=text\n  radio_group field=2 [\"Morning\", \"Evening\"]\n  input field=3 max_len=4 mode=digits placeholder=\"PIN\"\n  checkbox field=4 \"I agree\"\n  input field=5 max_len=64 mode=email\n  input field=6 max_len=64 mode=digits\n  input field=7 max_len=64 mode=text\n"
    );
    let module = ok(&source);
    let fields: Vec<&str> = module.screens[0]
        .fields()
        .iter()
        .map(|f| f.source())
        .collect();
    assert_eq!(
        fields,
        ["name", "when", "pin", "agree", "mail", "tel", "any"]
    );
}

#[test]
fn form_control_props_outside_the_subset_are_refused_where_written() {
    // Subset v1: each refused at the marker's first character, on
    // line 2 of `props_screen`.
    let nine = "[\"1\", \"2\", \"3\", \"4\", \"5\", \"6\", \"7\", \"8\", \"9\"]";
    let nine_options = format!("<RadioGroup id=\"r\" options={{{nine}}} />");
    for (body, code, marker) in [
        // text inputs
        (
            "<TextInput id=\"a\" maxLength={0} />",
            Code::InvalidValue,
            "0}",
        ),
        (
            "<TextInput id=\"a\" maxLength={65} />",
            Code::InvalidValue,
            "65",
        ),
        (
            "<TextInput id=\"a\" maxLength={1.5} />",
            Code::InvalidValue,
            "1.5",
        ),
        (
            "<TextInput id=\"a\" maxLength=\"10\" />",
            Code::InvalidValue,
            "\"10",
        ),
        (
            "<TextInput id=\"a\" maxLength={props.n} />",
            Code::InvalidValue,
            "props.n",
        ),
        (
            "<TextInput id=\"a\" maxLength={LEN} />",
            Code::InvalidValue,
            "LEN",
        ),
        (
            "<TextInput id=\"a\" inputMode=\"decimal\" />",
            Code::InvalidValue,
            "\"decimal",
        ),
        (
            "<TextInput id=\"a\" inputMode={MODE} />",
            Code::InvalidValue,
            "inputMode",
        ),
        (
            "<TextInput id=\"a\" type=\"password\" />",
            Code::UnknownProp,
            "type",
        ),
        (
            "<TextInput id=\"a\" required />",
            Code::UnknownProp,
            "required",
        ),
        // checkboxes
        (
            "<Checkbox label=\"Agree\" />",
            Code::MissingProp,
            "<Checkbox",
        ),
        ("<Checkbox id=\"c\" />", Code::MissingProp, "<Checkbox"),
        (
            "<Checkbox id=\"c\" label={props.text} />",
            Code::InvalidValue,
            "label",
        ),
        (
            "<Checkbox id=\"c\" label=\"Agree\" checked />",
            Code::UnknownProp,
            "checked",
        ),
        (
            "<Checkbox id=\"c\" label=\"Agree\">x</Checkbox>",
            Code::InvalidChildren,
            "x</",
        ),
        // radio groups
        (
            "<RadioGroup options={[\"a\"]} />",
            Code::MissingProp,
            "<RadioGroup",
        ),
        ("<RadioGroup id=\"r\" />", Code::MissingProp, "<RadioGroup"),
        (
            "<RadioGroup id=\"r\" options={[]} />",
            Code::InvalidValue,
            "[]",
        ),
        (nine_options.as_str(), Code::Limit, "[\"1"),
        (
            "<RadioGroup id=\"r\" options={[\"a\", props.b]} />",
            Code::Unsupported,
            "props.b",
        ),
        (
            "<RadioGroup id=\"r\" options={[\"a\", ...props.b]} />",
            Code::Unsupported,
            "...props",
        ),
        (
            "<RadioGroup id=\"r\" options={[\"a\", , \"b\"]} />",
            Code::Unsupported,
            ", \"b",
        ),
        (
            "<RadioGroup id=\"r\" options={[\"\"]} />",
            Code::InvalidValue,
            "\"\"",
        ),
        (
            "<RadioGroup id=\"r\" options={props.list} />",
            Code::InvalidValue,
            "props.list",
        ),
        (
            "<RadioGroup id=\"r\" options=\"a\" />",
            Code::InvalidValue,
            "options",
        ),
        (
            "<RadioGroup id=\"r\" options={[\"a\"]} value=\"a\" />",
            Code::UnknownProp,
            "value",
        ),
    ] {
        let source =
            props_screen(body).replace("export function", "const LEN = 65;\nexport function");
        // The token line moves the screen to line 3.
        let all = errors(&source);
        let loc = at(3, column_of(body, marker));
        assert!(
            all.iter().any(|e| e.code == code && e.loc == loc),
            "{body}: expected {code:?} at {loc:?}, got {all:#?}"
        );
    }
    // A list picker's options, more than a radio group holds, are named in
    // the help.
    let e = errors(&props_screen(&nine_options))
        .into_iter()
        .find(|e| e.code == Code::Limit)
        .unwrap();
    assert!(e.help.as_ref().unwrap().contains("<Select>"));
    // A masked field is never steered towards a plain one.
    let e = error(&props_screen("<TextInput id=\"pin\" type=\"password\" />"));
    let help = e.help.unwrap();
    assert!(
        help.contains("masked entry") && help.contains("do not remove `type`"),
        "{help}"
    );
    let e = error(&props_screen("<TextInput id=\"a\" pattern=\"[0-9]*\" />"));
    assert!(e.help.unwrap().contains("inputMode"));
    // A label or an option holds at most 256 bytes.
    let long = "x".repeat(257);
    let e = error(&props_screen(&format!(
        "<Checkbox id=\"c\" label=\"{long}\" />"
    )));
    assert_eq!(e.code, Code::Limit);
    let e = error(&props_screen(&format!(
        "<RadioGroup id=\"r\" options={{[\"{long}\"]}} />"
    )));
    assert_eq!(e.code, Code::Limit);
}

#[test]
fn props_whose_names_give_one_group_or_slot_name_are_refused() {
    // `isReady` and `is_ready`, or `ready` and `Ready`, would share a
    // GroupId or SlotId in generated code: refused where the second is.
    for (body, marker, what) in [
        (
            "<Column>{props.isReady && <Text>a</Text>}{props.is_ready && <Text>b</Text>}</Column>",
            "props.is_ready",
            "group",
        ),
        (
            "<Column>{props.ready && <Text>a</Text>}{props.Ready ? <Text>b</Text> : null}</Column>",
            "props.Ready",
            "group",
        ),
        (
            "<Row><Text>{props.isReady}</Text><Text>{props.is_ready}</Text></Row>",
            "<Text>{props.is_ready",
            "slot",
        ),
    ] {
        let e = error(&props_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (Code::DuplicateName, at(2, column_of(body, marker))),
            "{body}"
        );
        assert!(
            e.message
                .contains(&format!("would get the same {what} name")),
            "{}",
            e.message
        );
    }
    // One prop showing two elements shares its group: not a clash.
    ok(&props_screen(
        "<Column>{props.ready && <Text>a</Text>}{props.ready && <Text>b</Text>}</Column>",
    ));
}

#[test]
fn groups_count_towards_the_depth_and_node_limits() {
    let mut options = Options::new("test.tsx");
    options.limits = Limits {
        max_depth: 2,
        ..Limits::default()
    };
    // Column (1), group (2), text (3): one level too deep.
    let source = props_screen("<Column>{props.a && <Text>x</Text>}</Column>");
    let analysis = run_with(&source, &options);
    assert!(analysis.module.is_none());
    assert!(analysis.diagnostics.iter().any(|d| d.code == Code::Limit));
    options.limits = Limits {
        max_nodes: 2,
        ..Limits::default()
    };
    let analysis = run_with(&source, &options);
    assert!(analysis.diagnostics.iter().any(|d| d.code == Code::Limit));
}

#[test]
fn component_bodies_are_a_single_return() {
    let source = format!(
        "{KIT}export function S() {{\n  const [v, setV] = useState(0);\n  if (v) {{}}\n  return <Screen><Text>x</Text></Screen>;\n}}\n"
    );
    let all = errors(&source);
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].loc, at(3, 3));
    assert!(all[0].message.contains("React hook `useState`"));
    assert!(all[0].help.as_ref().unwrap().contains("state"));
    let e = error(&format!("{KIT}export function S() {{ return <Row />; }}\n"));
    assert_eq!(e.code, Code::Unsupported);
    let e = error(&format!(
        "{KIT}export async function S() {{ return <Screen><Text>x</Text></Screen>; }}\n"
    ));
    assert_eq!(e.code, Code::Unsupported);
    let e = error(&format!(
        "{KIT}export default () => <Screen><Text>x</Text></Screen>;\n"
    ));
    assert!(e.message.contains("must be named"));
}

#[test]
fn a_web_style_root_is_reported_with_everything_inside_it() {
    let source = format!(
        "{KIT}export function S() {{\n  return (\n    <div>\n      <Button>Go</Button>\n      <span>{{name}}</span>\n    </div>\n  );\n}}\n"
    );
    let found: Vec<(Code, Option<Loc>)> = errors(&source).iter().map(|d| (d.code, d.loc)).collect();
    assert_eq!(
        found,
        [
            (Code::UnknownElement, at(4, 5)),
            (Code::MissingProp, at(5, 7)),
            (Code::UnknownElement, at(6, 7)),
            (Code::Unsupported, at(6, 14)),
        ]
    );
}

#[test]
fn imports_other_than_the_kit_are_reported() {
    let source = format!(
        "import {{ useState }} from \"react\";\nimport \"./styles.css\";\nimport {{ Panel }} from \"./panel\";\nimport * as Kit from \"nowayos:kit\";\n{}",
        screen("<Text>x</Text>")
    );
    let found: Vec<(Code, Option<Loc>)> = errors(&source).iter().map(|d| (d.code, d.loc)).collect();
    assert_eq!(
        found,
        [
            (Code::Unsupported, at(1, 10)),
            (Code::Unsupported, at(2, 1)),
            (Code::Unsupported, at(3, 1)),
            (Code::Unsupported, at(4, 8)),
        ]
    );
    let e = error(&format!(
        "import {{ Panel }} from \"nowayos:kit\";\n{}",
        screen("<Text>x</Text>")
    ));
    assert_eq!(e.code, Code::UnknownName);
}

#[test]
fn a_file_without_a_screen_is_reported() {
    let e = error("const GAP = 8;\n");
    assert_eq!((e.code, e.loc), (Code::NoScreen, None));
}

#[test]
fn limits_are_enforced() {
    let limits = Limits {
        max_depth: 2,
        max_nodes: 3,
        max_text_bytes: 5,
    };
    let options = Options {
        limits,
        ..Options::new("test.tsx")
    };
    let check = |body: &str| -> Vec<Code> {
        let analysis = run_with(&screen(body), &options);
        assert!(analysis.module.is_none());
        analysis.diagnostics.iter().map(|d| d.code).collect()
    };
    assert_eq!(
        check("<Column><Column><Text>x</Text></Column></Column>"),
        [Code::Limit]
    );
    assert_eq!(
        check("<Row><Spacer /><Spacer /><Spacer /></Row>"),
        [Code::Limit]
    );
    assert_eq!(check("<Text>too long</Text>"), [Code::Limit]);
    assert_eq!(
        check("<TextInput id=\"a\" placeholder=\"too long\" />"),
        [Code::Limit]
    );
}

#[test]
fn the_default_depth_limit_keeps_screens_within_the_runtime() {
    // `levels` elements deep: Text inside (levels - 1) Columns.
    let nest = |levels: usize| {
        let mut body = "<Text>x</Text>".to_owned();
        for _ in 1..levels {
            body = format!("<Column>{body}</Column>");
        }
        body
    };
    ok(&screen(&nest(16)));
    let e = error(&screen(&nest(17)));
    assert_eq!(e.code, Code::Limit);
    assert!(e.message.contains("16 levels"), "{}", e.message);
}

#[test]
fn syntax_errors_stop_before_analysis() {
    let analysis = run("export function S() { return <Screen>; }");
    assert!(analysis.module.is_none());
    assert!(analysis.diagnostics.iter().all(|d| d.code == Code::Syntax));
}

/// `(code, position)` of every error, in order.
fn found(source: &str) -> Vec<(Code, Option<Loc>)> {
    errors(source).iter().map(|d| (d.code, d.loc)).collect()
}

#[test]
fn control_and_invisible_characters_in_ui_text_are_refused_once_however_written() {
    // Line 2 of `screen(…)` starts the body at column 57.
    let cases: [(&str, u32); 8] = [
        // Raw in JSX text, and as a reference: the parser front end, exactly
        // where the character is.
        ("<Text>Total: 10\u{202e}00</Text>", 72),
        ("<Text>a&#x202E;b</Text>", 64),
        ("<Text>a\u{1b}[2Jb</Text>", 64),
        // In a JSX attribute string, raw or as a reference.
        ("<TextInput id=\"n\" placeholder=\"x\u{2066}y\" />", 89),
        ("<TextInput id=\"n\" placeholder=\"x&#8203;y\" />", 89),
        // As a JavaScript escape in a string literal: the analyser, at the
        // literal.
        ("<Text>{\"a\\u202Eb\"}</Text>", 64),
        ("<Button onPress={onGo}>{\"Pay \\u202EKCOLB\"}</Button>", 81),
        (
            "<TextInput id=\"n\" placeholder={\"a\\u0007b\\u0000\"} />",
            88,
        ),
    ];
    for (body, column) in cases {
        assert_eq!(
            found(&screen(body)),
            [(Code::Unsupported, at(2, column))],
            "{body:?}"
        );
    }
    // A JavaScript NUL escape is valid syntax, but still a control character.
    assert_eq!(
        found(&screen("<Text>{\"a\\0b\"}</Text>")),
        [(Code::Unsupported, at(2, 64))]
    );
    // Visible Latin-1 text is fine, however it is written.
    let module = ok(&screen(
        "<Column><Text>Caf\u{e9} \u{b7} 5 \u{a3}</Text><Text>{\"Caf\\u00e9\"}</Text><TextInput id=\"n\" placeholder=\"Pr&#233;nom\" /></Column>",
    ));
    assert_eq!(
        outline(&module, &module.screens[0]),
        "column gap=0 padding=0\n  text \"Caf\u{e9} \u{b7} 5 \u{a3}\"\n  text \"Caf\u{e9}\"\n  input field=1 max_len=64 mode=text placeholder=\"Pr\u{e9}nom\"\n"
    );
}

#[test]
fn text_roles_and_weights_reach_the_ir() {
    // `role` and `weight` on <Text>; display, title and heading are
    // Bold, the rest Regular unless `weight="bold"`.
    let source = screen(
        "<Column><Text role=\"title\">Welcome</Text><Text role=\"display\">A047</Text><Text weight=\"bold\">Bold body</Text><Text role=\"caption\" weight=\"bold\">Small</Text><Text role=\"label\">Label</Text><Text role=\"body\" weight=\"regular\">Plain</Text><Text role=\"heading\" weight=\"bold\">Head</Text></Column>",
    );
    assert_eq!(
        ok_outline(&source),
        "column gap=0 padding=0\n  text \"Welcome\" role=title weight=bold\n  text \"A047\" role=display weight=bold\n  text \"Bold body\" role=body weight=bold\n  text \"Small\" role=caption weight=bold\n  text \"Label\" role=label weight=regular\n  text \"Plain\"\n  text \"Head\" role=heading weight=bold\n"
    );
    // A slot in a role: the application's text, checked when it is set.
    assert_eq!(
        ok_outline(&props_screen(
            "<Text role=\"display\">{props.ticket}</Text>"
        )),
        "slot 1 role=display weight=bold\n"
    );
    // Roles and weights outside the set, or not literal, are refused where
    // written; so is Regular for a role drawn only in Bold, and a role on
    // anything but <Text>.
    for (body, code, marker) in [
        (
            "<Text role=\"headline\">x</Text>",
            Code::InvalidValue,
            "\"headline",
        ),
        ("<Text role={TITLE}>x</Text>", Code::InvalidValue, "role="),
        ("<Text weight=\"700\">x</Text>", Code::InvalidValue, "\"700"),
        (
            "<Text role=\"title\" weight=\"regular\">x</Text>",
            Code::InvalidValue,
            "\"regular",
        ),
        (
            "<Text role=\"display\" weight=\"regular\">{props.ticket}</Text>",
            Code::InvalidValue,
            "\"regular",
        ),
        (
            "<Button role=\"title\" onPress={props.onGo}>Go</Button>",
            Code::UnknownProp,
            "role",
        ),
    ] {
        let e = error(&props_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}"
        );
    }
}

#[test]
fn an_icon_is_named_from_the_nowayos_set() {
    // `<Icon name="…">`, a literal from the set.
    assert_eq!(
        ok_outline(&screen(
            "<Row><Icon name=\"fuel\" /><Icon name=\"user-check\" /></Row>"
        )),
        "row gap=0 padding=0\n  icon fuel\n  icon user-check\n"
    );
    for (body, code, marker) in [
        ("<Icon />", Code::MissingProp, "<Icon"),
        ("<Icon name=\"rocket\" />", Code::InvalidValue, "\"rocket"),
        ("<Icon name={props.ticket} />", Code::InvalidValue, "name="),
        ("<Icon name=\"fuel\">x</Icon>", Code::InvalidChildren, "x<"),
    ] {
        let e = error(&props_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}"
        );
    }
}

#[test]
fn a_progress_bar_takes_a_prop_numbered_with_the_slots() {
    // `<ProgressBar value={…}>` from a prop, numbered with the
    // slots, destructured or not.
    assert_eq!(
        ok_outline(&props_screen(
            "<Column><Text>{props.ticket}</Text><ProgressBar value={props.fuelled} /></Column>"
        )),
        "column gap=0 padding=0\n  slot 1\n  progress slot 2\n"
    );
    // No value, a literal, the props object, a prop already a slot, or
    // children: refused where written.
    for (body, code, marker) in [
        ("<ProgressBar />", Code::MissingProp, "<ProgressBar"),
        ("<ProgressBar value={40} />", Code::InvalidValue, "40"),
        ("<ProgressBar value=\"40\" />", Code::InvalidValue, "\"40"),
        (
            "<ProgressBar value={props} />",
            Code::InvalidValue,
            "props}",
        ),
        (
            "<Column><Text>{props.ticket}</Text><ProgressBar value={props.ticket} /></Column>",
            Code::DuplicateName,
            "<ProgressBar",
        ),
        (
            "<ProgressBar value={props.fuelled}>x</ProgressBar>",
            Code::InvalidChildren,
            "x<",
        ),
    ] {
        let e = error(&props_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}"
        );
    }
}

#[test]
fn text_lines_reach_the_ir_and_more_than_eight_are_refused() {
    // `lines` on <Text>, a literal from 1 to 8, on static text and
    // on a slot, with a role and weight or without.
    let source = screen(
        "<Column><Text lines={3}>A long notice</Text><Text role=\"caption\" weight=\"bold\" lines={8}>Small print</Text><Text lines={1}>One line</Text></Column>",
    );
    assert_eq!(
        ok_outline(&source),
        "column gap=0 padding=0\n  text \"A long notice\" lines=3\n  text \"Small print\" role=caption weight=bold lines=8\n  text \"One line\"\n"
    );
    assert_eq!(
        ok_outline(&props_screen("<Text lines={2}>{props.ticket}</Text>")),
        "slot 1 lines=2\n"
    );
    // More than 8, none, a fraction, a leading zero, not a number, or on
    // anything but <Text>: refused where written.
    for (body, code, marker) in [
        ("<Text lines={9}>x</Text>", Code::InvalidValue, "9}"),
        ("<Text lines={0}>x</Text>", Code::InvalidValue, "0}"),
        ("<Text lines={2.5}>x</Text>", Code::InvalidValue, "2.5"),
        ("<Text lines={03}>x</Text>", Code::InvalidValue, "03"),
        ("<Text lines=\"3\">x</Text>", Code::InvalidValue, "\"3"),
        (
            "<Text lines={props.ticket}>x</Text>",
            Code::InvalidValue,
            "props.ticket}",
        ),
        (
            "<Button lines={2} onPress={props.onGo}>Go</Button>",
            Code::UnknownProp,
            "lines",
        ),
    ] {
        let e = error(&props_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}"
        );
    }
}

#[test]
fn cards_notices_header_bars_and_button_variants_reach_the_ir() {
    // Surfaces as columns and a row, and a button's variant.
    let source = screen(
        "<Column><HeaderBar padding={8}><Text role=\"heading\">Desk</Text><Button variant=\"secondary\" onPress={onBack}>Home</Button></HeaderBar><Card gap={12}><Button variant=\"quiet\" onPress={onBack}>Cancel</Button><Button variant=\"primary\" onPress={onGo}>Go</Button></Card><Notice tone=\"caution\"><Text>Note</Text></Notice><Notice tone=\"success\" padding={4}><Text>Done</Text></Notice><Notice tone=\"warning\"><Text weight=\"bold\">Careful</Text></Notice></Column>",
    );
    assert_eq!(
        ok_outline(&source),
        "column gap=0 padding=0\n  row gap=0 padding=8 surface=header\n    text \"Desk\" role=heading weight=bold\n    button \"Home\" action=1 variant=secondary\n  column gap=12 padding=0 surface=card\n    button \"Cancel\" action=1 variant=quiet\n    button \"Go\" action=2\n  column gap=0 padding=0 surface=notice-caution\n    text \"Note\"\n  column gap=0 padding=4 surface=notice-success\n    text \"Done\"\n  column gap=0 padding=0 surface=notice-warning\n    text \"Careful\" role=body weight=bold\n"
    );
    // Values outside the set, or not literal, a notice without its tone,
    // and the props on the wrong element are refused where written.
    for (body, code, marker) in [
        (
            "<Button variant=\"danger\" onPress={onGo}>Go</Button>",
            Code::InvalidValue,
            "\"danger",
        ),
        (
            "<Button variant={onGo} onPress={onGo}>Go</Button>",
            Code::InvalidValue,
            "variant",
        ),
        (
            "<Notice><Text>x</Text></Notice>",
            Code::MissingProp,
            "<Notice",
        ),
        (
            "<Notice tone=\"info\"><Text>x</Text></Notice>",
            Code::InvalidValue,
            "\"info",
        ),
        (
            "<Text variant=\"quiet\">x</Text>",
            Code::UnknownProp,
            "variant",
        ),
        (
            "<Card tone=\"success\"><Text>x</Text></Card>",
            Code::UnknownProp,
            "tone",
        ),
        (
            "<HeaderBar>loose</HeaderBar>",
            Code::InvalidChildren,
            "loose",
        ),
    ] {
        let e = error(&screen(body));
        let column = 57 + u32::try_from(body.find(marker).unwrap()).unwrap();
        assert_eq!((e.code, e.loc), (code, at(2, column)), "{body}");
    }
}

#[test]
fn characters_outside_a_roles_glyph_atlases_are_refused_naming_where() {
    // The device draws text from glyph atlases made at build time -
    // Latin-1, and for the display role digits, capitals and a few signs -
    // so any other character is refused when the screen is converted,
    // naming the line and column of the text and the character's place in
    // it.
    for (body, marker, character) in [
        (
            "<Text role=\"display\">A047a</Text>",
            "A047a",
            "'a' (U+0061), character 5 ",
        ),
        (
            "<Text role=\"display\">{\"\u{20ac}5\"}</Text>",
            "\"\u{20ac}",
            "'\u{20ac}' (U+20AC), character 1 ",
        ),
        (
            "<Text>Pay 5 \u{20ac}</Text>",
            "Pay",
            "'\u{20ac}' (U+20AC), character 7 ",
        ),
        (
            "<Text role=\"title\">Next \u{2192}</Text>",
            "Next",
            "'\u{2192}' (U+2192), character 6 ",
        ),
        (
            "<Button onPress={props.onGo}>Go \u{2014} now</Button>",
            "Go \u{2014}",
            "'\u{2014}' (U+2014), character 4 ",
        ),
        (
            "<TextInput id=\"n\" placeholder=\"\u{2026}\" />",
            "\"\u{2026}",
            "'\u{2026}' (U+2026), character 1 ",
        ),
        (
            "<Checkbox id=\"c\" label=\"\u{2713} Yes\" />",
            "\"\u{2713}",
            "'\u{2713}' (U+2713), character 1 ",
        ),
        (
            "<RadioGroup id=\"r\" options={[\"A\", \"\u{3b2}\"]} />",
            "\"\u{3b2}",
            "'\u{3b2}' (U+03B2), character 1 ",
        ),
    ] {
        let e = error(&props_screen(body));
        let prefix = "export function S(props: P) { return <Screen>".len();
        let column =
            u32::try_from(prefix + body[..body.find(marker).unwrap()].chars().count() + 1).unwrap();
        assert_eq!(
            (e.code, e.loc),
            (Code::Unsupported, at(2, column)),
            "{body}"
        );
        assert!(e.message.contains(character), "{body}: {}", e.message);
        let help = e.help.unwrap();
        if body.contains("display") {
            assert!(help.contains("digits, the capitals"), "{help}");
        } else {
            assert!(help.contains("Latin-1"), "{help}");
        }
    }
    // Everything the display role draws, and a body text using all of
    // Latin-1's printable characters, convert.
    let display = " #%+,-./0123456789:ABCDEFGHIJKLMNOPQRSTUVWXYZ\u{a3}";
    let latin1: String = (' '..='~')
        .chain('\u{a1}'..='\u{ff}')
        .filter(|&c| !matches!(c, '\u{ad}' | '{' | '}' | '<' | '>' | '&'))
        .collect();
    let module = ok(&props_screen(&format!(
        "<Column><Text role=\"display\">{display}</Text><Text>{latin1}</Text></Column>"
    )));
    assert_eq!(module.screens[0].nodes().len(), 3);
}

#[test]
fn attribute_strings_with_line_breaks_are_refused_once() {
    // Babel would render "First name and last name", TypeScript keeps the
    // line break and indentation: report, never pick one.
    let source = format!(
        "{KIT}export function S() {{ return <Screen><TextInput id=\"name\" placeholder=\"First name\n        and last name\" /></Screen>; }}\n"
    );
    let e = error(&source);
    assert_eq!((e.code, e.loc), (Code::Unsupported, at(2, 82)));
    assert!(e.message.contains("Babel"), "{}", e.message);
    // A string literal says exactly what it means: allowed on one line.
    ok(&format!(
        "{KIT}export function S() {{ return <Screen><TextInput id=\"name\" placeholder={{\"First and last name\"}} /></Screen>; }}\n"
    ));
}

#[test]
fn attribute_strings_that_fail_conversion_are_reported_once() {
    // The bad reference is the only problem: no follow-on error about the
    // value it would have had.
    for body in [
        "<TextInput id=\"a&copy;\" />",
        "<Column gap=\"&copy;\" />",
        "<TextInput id=\"n\" placeholder=\"&copy;\" />",
        "<Button onPress=\"&copy;\">Go</Button>",
    ] {
        let found = found(&screen(body));
        assert_eq!(found.len(), 1, "{body}: {found:?}");
        assert_eq!(found[0].0, Code::Unsupported, "{body}");
    }
    // Text that fails conversion is still checked for where it is.
    assert_eq!(
        found(&screen("<Column>&copy; 2026</Column>")),
        [
            (Code::Unsupported, at(2, 65)),
            (Code::InvalidChildren, at(2, 65)),
        ]
    );
}

#[test]
fn imports_that_bind_no_values_still_load_their_module_and_are_reported() {
    let with = |import: &str| {
        format!(
            "{KIT}{import}\nexport function S() {{ return <Screen><Text>Hi</Text></Screen>; }}\n"
        )
    };
    let e = error(&with("import {} from \"./global.css\";"));
    assert_eq!((e.code, e.loc), (Code::Unsupported, at(2, 1)));
    assert!(e.help.unwrap().contains("CSS"));
    let e = error(&with("import { type Theme } from \"./theme.css\";"));
    assert_eq!((e.code, e.loc), (Code::Unsupported, at(2, 1)));
    assert!(e.help.unwrap().contains("import type"));
    // A whole-declaration type import is erased by every compiler.
    ok(&with("import type { Theme } from \"./theme\";"));
    ok(&with("import type * as Themes from \"./theme\";"));
}

#[test]
fn legacy_octal_numbers_and_escapes_are_syntax_errors() {
    let body = "export function S() { return <Screen><Text>Hi</Text></Screen>; }\n";
    for number in ["010", "00", "08", "09.5"] {
        let e = error(&format!("{KIT}const GAP = {number};\n{body}"));
        assert_eq!((e.code, e.loc), (Code::Syntax, at(2, 13)), "{number}");
    }
    let e = error(&screen("<Column gap={010}><Text>x</Text></Column>"));
    assert_eq!((e.code, e.loc), (Code::Syntax, at(2, 70)));
    // Fine: zero, an exponent, a separator, a modern octal literal.
    ok(&format!(
        "{KIT}const A = 0;\nconst B = 0e1;\nconst C = 1_000;\nconst D = 0o10;\n{body}"
    ));
    // `\8` is an escape strict mode refuses; the value would be "8".
    assert_eq!(
        found(&screen("<Text>{\"\\8\"}</Text>")),
        [(Code::Syntax, at(2, 65))]
    );
    // A legacy octal escape of a control character is also that character.
    assert_eq!(
        found(&screen("<Text>{\"a\\01\"}</Text>")),
        [(Code::Unsupported, at(2, 64)), (Code::Syntax, at(2, 66))]
    );
}

#[test]
fn names_declared_twice_are_refused_once() {
    let body = "export function S() { return <Screen><Text>Hi</Text></Screen>; }\n";
    // A screen redeclaring an imported kit component.
    let source = "import { Column, Screen, Text } from \"nowayos:kit\";\nexport function Column() { return <Screen><Text>Hi</Text></Screen>; }\n";
    assert_eq!(found(source), [(Code::DuplicateName, at(2, 8))]);
    // The same name imported twice.
    let source = format!("{KIT}import {{ Text }} from \"nowayos:kit\";\n{body}");
    assert_eq!(found(&source), [(Code::DuplicateName, at(2, 10))]);
    // A token declared before the import of the same name.
    let source = format!("const Text = 5;\n{KIT}{body}");
    let e = error(&source);
    let text = u32::try_from(KIT.find(" Text,").unwrap() + 2).unwrap();
    assert_eq!((e.code, e.loc), (Code::DuplicateName, at(2, text)));
    // Two constants, or two functions, with the same name: one error each,
    // not a second report from the IR's own clash check.
    let source = format!("{KIT}const A = 1;\nconst A = 2;\n{body}");
    assert_eq!(found(&source), [(Code::DuplicateName, at(3, 7))]);
    let source = format!("{KIT}function S() {{ return <Screen><Text>a</Text></Screen>; }}\n{body}");
    assert_eq!(found(&source), [(Code::DuplicateName, at(3, 8))]);
    // A type alias and a constant may share a name: different namespaces.
    ok(&format!("{KIT}type GAP = number;\nconst GAP = 4;\n{body}"));
}

#[test]
fn exports_must_name_declarations_once() {
    let body = "export function S() { return <Screen><Text>Hi</Text></Screen>; }\n";
    let with = |tail: &str| format!("{KIT}{body}{tail}\n");
    assert_eq!(
        found(&with("export { AlsoMissing };")),
        [(Code::UnknownName, at(3, 10))]
    );
    assert_eq!(
        found(&with("export default Missing;")),
        [(Code::UnknownName, at(3, 16))]
    );
    assert_eq!(
        found(&with("type P = {};\nexport default P;")),
        [(Code::InvalidValue, at(4, 16))]
    );
    assert_eq!(
        found(&with("export { S };")),
        [(Code::DuplicateName, at(3, 10))]
    );
    assert_eq!(
        found(&with("export default S;\nexport { S as default };")),
        [(Code::DuplicateName, at(4, 10))]
    );
    // Fine: exporting a declared screen, a type, an import, under two names,
    // and before the declaration.
    ok(&format!(
        "{KIT}type P = {{}};\nexport {{ P, Text, S as Other }};\nexport default S;\n{body}"
    ));
}

#[test]
fn parameters_bound_twice_or_with_reserved_names_are_refused() {
    let with = |params: &str| {
        format!("{KIT}export function S({params}) {{ return <Screen><Text>Hi</Text></Screen>; }}\n")
    };
    assert_eq!(
        found(&with("{ onGo, onGo }")),
        [(Code::DuplicateName, at(2, 27))]
    );
    assert_eq!(
        found(&with("{ onGo, onBack: onGo }")),
        [(Code::DuplicateName, at(2, 27))]
    );
    assert_eq!(found(&with("{ eval }")), [(Code::Syntax, at(2, 21))]);
    assert_eq!(found(&with("arguments")), [(Code::Syntax, at(2, 19))]);
}

#[test]
fn names_strict_mode_reserves_cannot_be_declared() {
    let body = "export function S() { return <Screen><Text>Hi</Text></Screen>; }\n";
    for name in ["implements", "let", "static", "yield", "eval", "arguments"] {
        let e = error(&format!("{KIT}const {name} = 1;\n{body}"));
        assert_eq!((e.code, e.loc), (Code::Syntax, at(2, 7)), "{name}");
    }
    let e = error(&format!(
        "import {{ Screen, Text, Spacer as interface }} from \"nowayos:kit\";\n{body}"
    ));
    assert_eq!((e.code, e.loc), (Code::Syntax, at(1, 24)));
}

fn config() -> Config {
    Config {
        cases: 256,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(0x616e_616c_7973_6531),
        ..Config::default()
    }
}

/// One child of a generated screen's root `<Column>`: inside subset v0, or a
/// construct outside it with the error it must produce and where.
struct Piece {
    /// The TSX, on one line; `{n}` becomes the piece's index (unique ids).
    tsx: &'static str,
    /// Elements it adds to the IR when supported.
    nodes: usize,
    /// For a construct outside subset v0: the code it must be reported with
    /// and the character offset in `tsx` it must be reported at.
    error: Option<(Code, u32)>,
}

const fn supported(tsx: &'static str, nodes: usize) -> Piece {
    Piece {
        tsx,
        nodes,
        error: None,
    }
}

const fn unsupported(tsx: &'static str, code: Code, offset: u32) -> Piece {
    Piece {
        tsx,
        nodes: 0,
        error: Some((code, offset)),
    }
}

const PIECES: [Piece; 35] = [
    supported("<Text>Hello</Text>", 1),
    supported("<Spacer />", 1),
    supported("<Spacer size={8} />", 1),
    supported("<Button onPress={onGo}>Go</Button>", 1),
    supported("<TextInput id=\"f{n}\" placeholder=\"Name\" />", 1),
    supported("<Row gap={4}><Text>a</Text><Spacer /></Row>", 3),
    supported(
        "<Column padding={2}><Button onPress={onBack}>Back</Button></Column>",
        2,
    ),
    // A colour value, not one of the theme's.
    unsupported(
        "<Text style={{ color: \"red\" }}>x</Text>",
        Code::InvalidValue,
        22,
    ),
    unsupported(
        "<Icon name=\"fuel\" style={{ width: 4 }} />",
        Code::UnknownProp,
        18,
    ),
    unsupported("<Spacer className=\"gap\" />", Code::UnknownProp, 8),
    supported("<Button onClick={onGo}>Go</Button>", 1),
    unsupported("<div>x</div>", Code::UnknownElement, 0),
    unsupported("<Text>{name}</Text>", Code::UnknownName, 7),
    unsupported("<Text>{onGo} and {onBack}</Text>", Code::Unsupported, 18),
    unsupported("{items.map((i) => <Text>{i}</Text>)}", Code::Unsupported, 1),
    supported("{onGo && <Text>x</Text>}", 2),
    supported("{onBack ? <Text>a</Text> : null}", 2),
    supported("{onGo ? <Spacer /> : <Row><Text>b</Text></Row>}", 4),
    unsupported("{onGo || <Text>x</Text>}", Code::Unsupported, 1),
    supported("{!onGo && <Text>x</Text>}", 2),
    supported("{onBack === \"a\" ? <Text>a</Text> : <Text>b</Text>}", 3),
    unsupported("{!!onGo && <Text>x</Text>}", Code::Unsupported, 1),
    unsupported("{onGo && \"text\"}", Code::InvalidChildren, 9),
    supported("<Button onClick={() => onGo()}>Go</Button>", 1),
    unsupported(
        "<Button onPress={() => { onGo(); onBack(); }}>Go</Button>",
        Code::Unsupported,
        17,
    ),
    unsupported("<Button>Go</Button>", Code::MissingProp, 0),
    unsupported("<TextInput placeholder=\"x\" />", Code::MissingProp, 0),
    supported("<Column gap=\"16px\" />", 1),
    unsupported("<Column gap=\"1rem\" />", Code::Unsupported, 12),
    unsupported("<Column {...rest} />", Code::Unsupported, 8),
    unsupported(
        "{onGo && <><Text>x</Text><Text>y</Text></>}",
        Code::Unsupported,
        9,
    ),
    unsupported("<Text>&copy; 2026</Text>", Code::Unsupported, 6),
    unsupported("<Text>{\"a\\u202Eb\"}</Text>", Code::Unsupported, 7),
    unsupported("<TextInput id=\"2fa{n}\" />", Code::InvalidValue, 14),
    unsupported("<Column gap={010} />", Code::Syntax, 13),
];

/// Pieces start on this line, one per line, indented by eight spaces.
const FIRST_PIECE_LINE: u32 = 6;

fn mixed_screen(picks: &[usize]) -> String {
    let mut body = String::new();
    for (n, &pick) in picks.iter().enumerate() {
        body.push_str("        ");
        body.push_str(&PIECES[pick].tsx.replace("{n}", &n.to_string()));
        body.push('\n');
    }
    format!(
        "{KIT}export function S({{ onGo, onBack }}: P) {{\n  return (\n    <Screen>\n      <Column>\n{body}      </Column>\n    </Screen>\n  );\n}}\n"
    )
}

proptest! {
    #![proptest_config(config())]

    /// "Never approximate", as a property of the analyser: a screen mixing
    /// subset-v0 elements with constructs outside the subset converts
    /// exactly when it has none of the latter, every one of them is reported
    /// with its code where it is written, and nothing else is reported.
    #[test]
    fn exactly_the_constructs_outside_the_subset_are_reported_where_written(
        picks in prop::collection::vec(0..PIECES.len(), 1..8)
    ) {
        let source = mixed_screen(&picks);
        let analysis = run(&source);
        let errors: Vec<&Diagnostic> =
            analysis.diagnostics.iter().filter(|d| d.is_error()).collect();
        let mut bad_lines = Vec::new();
        for (index, &pick) in picks.iter().enumerate() {
            let line = FIRST_PIECE_LINE + u32::try_from(index).unwrap();
            if let Some((code, offset)) = PIECES[pick].error {
                bad_lines.push(line);
                let loc = at(line, 9 + offset);
                prop_assert!(
                    errors.iter().any(|d| d.code == code && d.loc == loc),
                    "expected {:?} at {:?}\nsource:\n{}\nerrors: {:#?}", code, loc, source, errors
                );
            }
        }
        for d in &errors {
            prop_assert!(
                d.loc.is_some_and(|l| bad_lines.contains(&l.line)),
                "error away from every unsupported construct: {:?}\nsource:\n{}", d, source
            );
        }
        prop_assert_eq!(analysis.module.is_some(), bad_lines.is_empty(), "source:\n{}", source);
        if let Some(module) = &analysis.module {
            let expected: usize = 1 + picks.iter().map(|&p| PIECES[p].nodes).sum::<usize>();
            prop_assert_eq!(module.screens[0].nodes().len(), expected, "source:\n{}", source);
        }
    }
}

/// `KIT` with the theme's spacing tokens, `space`, plus a component
/// taking `props`, returning `<Screen>{body}</Screen>` on line 2.
fn spaced_screen(body: &str) -> String {
    let kit = KIT.replace(" } from", ", space } from");
    format!("{kit}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n")
}

#[test]
fn text_alignment_reaches_the_ir_and_other_values_are_refused() {
    // `align` on <Text>, start, center or end, on static text and on
    // a slot, with a role, weight and lines or without; start is the
    // default and named by nothing.
    assert_eq!(
        ok_outline(&props_screen(
            "<Column><Text align=\"center\">Hi</Text><Text role=\"title\" align=\"end\" lines={2}>Title</Text><Text align=\"start\">Plain</Text><Text align=\"center\">{props.note}</Text></Column>"
        )),
        "column gap=0 padding=0\n  text \"Hi\" align=center\n  text \"Title\" role=title weight=bold lines=2 align=end\n  text \"Plain\"\n  slot 1 align=center\n"
    );
    // Another value, not a literal, or on anything but <Text>: refused
    // where written.
    for (body, code, marker) in [
        (
            "<Text align=\"middle\">x</Text>",
            Code::InvalidValue,
            "\"middle",
        ),
        (
            "<Text align={props.side}>x</Text>",
            Code::InvalidValue,
            "align",
        ),
        (
            "<Button align=\"center\" onPress={props.onGo}>Go</Button>",
            Code::UnknownProp,
            "align",
        ),
    ] {
        let e = error(&props_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}"
        );
    }
}

#[test]
fn the_theme_s_spacing_tokens_reach_the_ir_and_unknown_ones_are_refused() {
    // `space.xs` to `space.xl`, from the kit, as a gap, padding or a
    // spacer's size; a number beside them stays a number.
    assert_eq!(
        ok_outline(&spaced_screen(
            "<Column gap={space.md} padding={space.lg}><Text>A</Text><Spacer size={space.xs} /><Card gap={8} padding={space.xl}><Text>B</Text></Card><Spacer size={space.sm} /></Column>"
        )),
        "column gap=space.md padding=space.lg\n  text \"A\"\n  spacer size=space.xs\n  column gap=8 padding=space.xl surface=card\n    text \"B\"\n  spacer size=space.sm\n"
    );
    // A token the theme has not got, the tokens unnamed, a token as a
    // number of bytes, and `space.md` without importing `space`: refused
    // where written.
    for (body, code, marker) in [
        (
            "<Column gap={space.huge}><Text>x</Text></Column>",
            Code::UnknownName,
            "space.huge",
        ),
        (
            "<Column gap={space}><Text>x</Text></Column>",
            Code::InvalidValue,
            "space}",
        ),
        (
            "<TextInput id=\"n\" maxLength={space.md} />",
            Code::InvalidValue,
            "space.md",
        ),
    ] {
        let e = error(&spaced_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
    let body = "<Column gap={space.md}><Text>x</Text></Column>";
    let e = error(&props_screen(body));
    assert_eq!(e.code, Code::Unsupported, "{e:?}");
    assert!(e.help.unwrap().contains("import { space }"));
}

#[test]
fn a_declared_style_reaches_the_ir() {
    // Every property a style may name, on a container, a text and
    // a button, each mapped onto the theme's tokens or layout units.
    assert_eq!(
        ok_outline(&spaced_screen(
            "<Column style={{ gap: space.sm, padding: 12, width: 360, height: 200, minWidth: 100, maxWidth: 480, alignItems: \"center\", justifyContent: \"space-between\", backgroundColor: \"input\", borderColor: \"input-border\", borderWidth: 2, borderRadius: 12 }}><Text style={{ color: \"text-muted\", textAlign: \"right\" }}>Note</Text><Button style={{ backgroundColor: \"text\", color: \"background\" }} onPress={props.onGo}>Go</Button></Column>"
        )),
        "column gap=space.sm padding=12 width=360 height=200 min-width=100 max-width=480 align=center justify=space-between fill=input border=input-border border-width=2 radius=12\n  text \"Note\" align=end colour=text-muted\n  button \"Go\" action=1 fill=text label=background\n"
    );
    // A card keeps its surface; a style names what it changes.
    assert_eq!(
        ok_outline(&spaced_screen(
            "<Card style={{ borderRadius: 4 }}><Text style={{ textAlign: \"left\" }}>A</Text></Card>"
        )),
        "column gap=0 padding=0 surface=card radius=4\n  text \"A\"\n"
    );
}

#[test]
fn everything_a_style_may_not_name_is_refused_where_written() {
    // Another property, a colour value, a value from a prop or
    // computed, a spread or computed key, a style that is not an object
    // literal, a property given twice, and values out of their range.
    for (body, code, marker) in [
        (
            // A gradient's property (contract v22): a colour value refused.
            "<Column style={{ background: \"#fff\" }}><Text>x</Text></Column>",
            Code::InvalidValue,
            "\"#fff",
        ),
        (
            "<Text style={{ fontSize: 20 }}>x</Text>",
            Code::UnknownProp,
            "fontSize",
        ),
        (
            "<Button style={{ borderRadius: 4 }} onPress={props.onGo}>Go</Button>",
            Code::UnknownProp,
            "borderRadius",
        ),
        (
            "<Column style={{ backgroundColor: \"#ffffff\" }}><Text>x</Text></Column>",
            Code::InvalidValue,
            "\"#ffffff",
        ),
        (
            "<Column style={{ borderColor: \"rgb(0, 0, 0)\" }}><Text>x</Text></Column>",
            Code::InvalidValue,
            "\"rgb",
        ),
        (
            "<Text style={{ color: props.tint }}>x</Text>",
            Code::InvalidValue,
            "props.tint",
        ),
        (
            "<Column style={{ ...props.look }}><Text>x</Text></Column>",
            Code::Unsupported,
            "...props",
        ),
        (
            "<Column style={{ [props.key]: 4 }}><Text>x</Text></Column>",
            Code::Unsupported,
            "[props",
        ),
        (
            "<Column style={props.look}><Text>x</Text></Column>",
            Code::InvalidValue,
            "props.look",
        ),
        (
            "<Column style=\"color: red\"><Text>x</Text></Column>",
            Code::InvalidValue,
            "\"color",
        ),
        (
            "<Column gap={8} style={{ gap: 4 }}><Text>x</Text></Column>",
            Code::DuplicateName,
            "style",
        ),
        (
            "<Text align=\"center\" style={{ textAlign: \"right\" }}>x</Text>",
            Code::DuplicateName,
            "style",
        ),
        (
            "<Column style={{ gap: 4, gap: 8 }}><Text>x</Text></Column>",
            Code::DuplicateName,
            "gap: 8",
        ),
        (
            "<Column style={{ borderWidth: 40 }}><Text>x</Text></Column>",
            Code::InvalidValue,
            "40",
        ),
        (
            "<Column style={{ width: space.md }}><Text>x</Text></Column>",
            Code::InvalidValue,
            "space.md",
        ),
        (
            "<Column style={{ alignItems: \"baseline\" }}><Text>x</Text></Column>",
            Code::InvalidValue,
            "\"baseline",
        ),
        (
            "<Column style={{ justifyContent: 2 }}><Text>x</Text></Column>",
            Code::InvalidValue,
            "2 }",
        ),
    ] {
        let e = error(&spaced_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
}

/// `KIT` with `ScrollView`, plus a component taking `props`,
/// returning `<Screen>{body}</Screen>` on line 2.
fn scrolling_screen(body: &str) -> String {
    let kit = KIT.replace(" } from", ", ScrollView } from");
    format!("{kit}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n")
}

#[test]
fn a_scroll_view_reaches_the_ir_with_its_one_child() {
    assert_eq!(
        ok_outline(&scrolling_screen(
            "<Column><Text>Title</Text><ScrollView id=\"answers\"><Column><Text>A</Text><Text>B</Text></Column></ScrollView></Column>"
        )),
        "column gap=0 padding=0\n  text \"Title\"\n  scroll\n    column gap=0 padding=0\n      text \"A\"\n      text \"B\"\n"
    );
}

#[test]
fn a_scroll_view_holds_exactly_one_child_and_may_hold_another() {
    // Any number in a screen, one inside another (contract v17); exactly
    // one child element.
    for body in [
        "<Column><ScrollView><Column><Text>A</Text></Column></ScrollView><ScrollView><Column><Text>B</Text></Column></ScrollView></Column>",
        "<ScrollView><Column><ScrollView><Column><Text>A</Text></Column></ScrollView></Column></ScrollView>",
    ] {
        let source = scrolling_screen(body);
        let analysis = run(&source);
        assert!(
            analysis.module.is_some(),
            "{body}: {:?}",
            analysis.diagnostics
        );
    }
    for (body, code, marker) in [
        (
            "<Column><ScrollView></ScrollView></Column>",
            Code::InvalidChildren,
            "<ScrollView>",
        ),
        (
            "<Column><ScrollView><Text>A</Text><Text>B</Text></ScrollView></Column>",
            Code::InvalidChildren,
            "<ScrollView>",
        ),
        (
            "<Column><ScrollView gap={4}><Column><Text>A</Text></Column></ScrollView></Column>",
            Code::UnknownProp,
            "gap",
        ),
        (
            "<Column style={{ overflow: \"hidden\" }}><Text>A</Text></Column>",
            Code::InvalidValue,
            "\"hidden\"",
        ),
    ] {
        let e = error(&scrolling_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
}

/// `KIT` with `Select`, plus a component taking `props`, returning
/// `<Screen>{body}</Screen>` on line 2.
fn picking_screen(body: &str) -> String {
    let kit = KIT.replace(" } from", ", Select } from");
    format!("{kit}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n")
}

#[test]
fn a_list_picker_reaches_the_ir() {
    // Options, a placeholder and how many rows show.
    assert_eq!(
        ok_outline(&picking_screen(
            "<Column><Select id=\"kind\" placeholder=\"Choose one\" options={[\"Bag\", \"Phone\", \"Keys\"]} rows={3} /><Select id=\"colour\" options={[\"Red\"]} /></Column>"
        )),
        "column gap=0 padding=0\n  list_picker field=1 rows=3 placeholder=\"Choose one\" [\"Bag\", \"Phone\", \"Keys\"]\n  list_picker field=2 rows=5 placeholder=\"\" [\"Red\"]\n"
    );
}

#[test]
fn a_list_picker_s_bounds_are_refused_where_written() {
    // Rows outside 3 to 8 or not a number, more than 64 options or
    // none, an option that is not a literal, no options, no id.
    let many = format!(
        "[{}]",
        (0..65)
            .map(|n| format!("\"Option {n}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let too_many = format!("<Select id=\"kind\" options={{{many}}} />");
    for (body, code, marker) in [
        (
            "<Select id=\"kind\" options={[\"A\"]} rows={2} />",
            Code::InvalidValue,
            "2}",
        ),
        (
            "<Select id=\"kind\" options={[\"A\"]} rows={9} />",
            Code::InvalidValue,
            "9}",
        ),
        (
            "<Select id=\"kind\" options={[\"A\"]} rows=\"5\" />",
            Code::InvalidValue,
            "\"5",
        ),
        (too_many.as_str(), Code::Limit, "[\"Option 0"),
        (
            "<Select id=\"kind\" options={[]} />",
            Code::InvalidValue,
            "[]",
        ),
        (
            "<Select id=\"kind\" options={[props.first]} />",
            Code::Unsupported,
            "props.first",
        ),
        ("<Select id=\"kind\" />", Code::MissingProp, "<Select"),
        ("<Select options={[\"A\"]} />", Code::MissingProp, "<Select"),
    ] {
        let e = error(&picking_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
}

/// `KIT` with `TextArea` and `Keyboard`, a design token `MOST` and a
/// component taking `props`, returning `<Screen>{body}</Screen>` on line 3.
fn writing_screen(body: &str) -> String {
    let kit = KIT.replace(" } from", ", TextArea, Keyboard } from");
    format!(
        "{kit}const MOST = 2000;\nexport function S(props: P) {{ return <Screen>{body}</Screen>; }}\n"
    )
}

#[test]
fn a_text_area_reaches_the_ir() {
    // Its rows - two when not given - most bytes, a number or a token, and
    // placeholder; a keyboard types into it.
    assert_eq!(
        ok_outline(&writing_screen(
            "<Column><TextArea id=\"note\" rows={4} maxLength={500} placeholder=\"Write here\" /><TextArea id=\"more\" maxLength={MOST} /><Keyboard /></Column>"
        )),
        "column gap=0 padding=0\n  text-area field=1 rows=4 max_len=500 placeholder=\"Write here\"\n  text-area field=2 rows=2 max_len=2000\n  keyboard\n"
    );
}

#[test]
fn a_text_area_s_bounds_are_refused_where_written() {
    // Rows outside 1 to 32, bytes outside 1 to 65,535, no maxLength, no
    // id, children.
    for (body, code, marker) in [
        (
            "<TextArea id=\"note\" rows={0} maxLength={10} />",
            Code::InvalidValue,
            "0}",
        ),
        (
            "<TextArea id=\"note\" rows={33} maxLength={10} />",
            Code::InvalidValue,
            "33}",
        ),
        (
            "<TextArea id=\"note\" maxLength={65536} />",
            Code::InvalidValue,
            "65536}",
        ),
        ("<TextArea id=\"note\" />", Code::MissingProp, "<TextArea"),
        (
            "<TextArea maxLength={10} />",
            Code::MissingProp,
            "<TextArea",
        ),
    ] {
        let e = error(&writing_screen(body));
        // Line 3: line 2 declares the token.
        assert_eq!(
            (e.code, e.loc),
            (code, at(3, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
}

#[test]
fn transitions_and_animations_reach_the_ir() {
    assert_eq!(
        ok_outline(&format!(
            "{KIT}export function S(props: P) {{ return <Screen><Card style={{{{ transition: \"opacity 0.2s ease-out\", animation: \"spin 800ms steps(8) infinite\" }}}}><Text>A</Text></Card></Screen>; }}\n"
        )),
        "column gap=0 padding=0 surface=card transition=opacity-200-0-ease-out animation=spin-800-0-steps-8-end-infinite\n  text \"A\"\n"
    );
}

#[test]
fn motion_the_runtime_cannot_run_is_refused_where_written() {
    for (style, marker) in [
        ("transition: \"color 2s\"", "\"color"),
        ("transition: \"opacity\"", "\"opacity"),
        ("animation: \"wobble 1s\"", "\"wobble"),
        ("animationIterationCount: \"0\"", "\"0"),
        ("transitionTimingFunction: \"bounce\"", "\"bounce"),
    ] {
        let body = format!("<Card style={{{{ {style} }}}}><Text>A</Text></Card>");
        let source =
            format!("{KIT}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n");
        let e = error(&source);
        assert_eq!(
            (e.code, e.loc),
            (Code::InvalidValue, at(2, column_of(&body, marker))),
            "{body}: {e:?}"
        );
    }
}

#[test]
fn shadows_gradients_and_opacity_reach_the_ir() {
    assert_eq!(
        ok_outline(&format!(
            "{KIT}export function S(props: P) {{ return <Screen><Card style={{{{ boxShadow: \"medium\", background: \"linear-gradient(45deg, var(--input), focus)\", opacity: 0.5 }}}}><Text>A</Text></Card></Screen>; }}\n"
        )),
        "column gap=0 padding=0 surface=card shadow=medium gradient=45-input-focus opacity=128\n  text \"A\"\n"
    );
}

#[test]
fn an_effect_s_bounds_are_refused_where_written() {
    for (style, marker) in [
        ("boxShadow: \"0 2px 4px black\"", "\"0 2px"),
        ("background: \"linear-gradient(red, blue)\"", "\"linear"),
        ("background: \"input\"", "\"input"),
        ("opacity: 2", "2 }"),
    ] {
        let body = format!("<Card style={{{{ {style} }}}}><Text>A</Text></Card>");
        let source =
            format!("{KIT}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n");
        let e = error(&source);
        assert_eq!(
            (e.code, e.loc),
            (Code::InvalidValue, at(2, column_of(&body, marker))),
            "{body}: {e:?}"
        );
    }
}

#[test]
fn a_text_with_inline_elements_is_rich() {
    assert_eq!(
        ok_outline(&format!(
            "{KIT}export function S(props: P) {{ return <Screen><Text>A <strong>b <em>c</em></strong> <code>d</code> <a onClick={{props.onGo}}>e</a>{{\" \"}}<span style={{{{ color: \"error\" }}}}>f</span></Text></Screen>; }}\n"
        )),
        "rich \"A \" \"b \" bold \"c\" bold italic \" \" \"d\" code \" \" \"e\" link=1 \" \" \"f\" colour=error\n"
    );
}

#[test]
fn a_rich_text_s_bounds_are_refused_where_written() {
    for (body, code, marker) in [
        ("<Text>A <u>b</u></Text>", Code::Unsupported, "<u"),
        (
            "<Text>A <a href=\"x\">b</a></Text>",
            Code::UnknownProp,
            "href",
        ),
        ("<Text>A <a>b</a></Text>", Code::MissingProp, "<a"),
        (
            "<Text>A <strong>{props.name}</strong></Text>",
            Code::Unsupported,
            "props.name",
        ),
        (
            "<Text lines={2}>A <b>b</b></Text>",
            Code::Unsupported,
            "<Text",
        ),
    ] {
        let source =
            format!("{KIT}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n");
        let e = error(&source);
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
}

/// `KIT` with the layers, plus a component taking `props`, returning
/// `<Screen>{body}</Screen>` on line 2.
fn layered_screen(body: &str) -> String {
    let kit = KIT.replace(" } from", ", Dialog, Menu, Popover, Tooltip } from");
    format!("{kit}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n")
}

#[test]
fn layers_reach_the_ir_beside_their_controls() {
    assert_eq!(
        ok_outline(&layered_screen(
            "<Column><Button id=\"more\" onPress={props.onMore}>More</Button><Menu for=\"more\" open={props.menuOpen}><Column><Button onPress={props.onCopy}>Copy</Button></Column></Menu><Tooltip for=\"more\">More choices</Tooltip><Dialog open={props.asking}><Text>Sure?</Text></Dialog></Column>"
        )),
        "column gap=0 padding=0\n  button \"More\" action=1\n  menu layer 1 for (button \"More\" action=1)\n    column gap=0 padding=0\n      button \"Copy\" action=2\n  tooltip \"More choices\" for (button \"More\" action=1)\n  dialog layer 2\n    text \"Sure?\"\n"
    );
}

#[test]
fn a_layer_s_bounds_are_refused_where_written() {
    // `for` naming nothing, an element after it or around it; no `open`
    // or `for`; two children; one prop opening two layers.
    let button = "<Button id=\"more\" onPress={props.onMore}>More</Button>";
    let after = format!(
        "<Column><Menu for=\"more\" open={{props.m}}><Text>A</Text></Menu>{button}</Column>"
    );
    let around =
        "<Column id=\"box\"><Popover for=\"box\" open={props.p}><Text>A</Text></Popover></Column>"
            .to_owned();
    let none = "<Column><Tooltip for=\"nothing\">Hi</Tooltip></Column>".to_owned();
    let no_open = format!("<Column>{button}<Menu for=\"more\"><Text>A</Text></Menu></Column>");
    let no_for = "<Column><Popover open={props.p}><Text>A</Text></Popover></Column>".to_owned();
    let two =
        "<Column><Dialog open={props.d}><Text>A</Text><Text>B</Text></Dialog></Column>".to_owned();
    let twice = "<Column><Dialog open={props.d}><Text>A</Text></Dialog><Dialog open={props.d}><Text>B</Text></Dialog></Column>".to_owned();
    for (body, code, marker) in [
        (after.as_str(), Code::UnknownName, "<Menu"),
        (around.as_str(), Code::UnknownName, "<Popover"),
        (none.as_str(), Code::UnknownName, "<Tooltip"),
        (no_open.as_str(), Code::MissingProp, "<Menu"),
        (no_for.as_str(), Code::MissingProp, "<Popover"),
        (two.as_str(), Code::InvalidChildren, "<Dialog"),
    ] {
        let e = error(&layered_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
    // The second dialog opened by the prop.
    let e = error(&layered_screen(&twice));
    assert_eq!(
        (e.code, e.loc),
        (
            Code::DuplicateName,
            at(2, column_of(&twice, "<Dialog open={props.d}><Text>B"))
        ),
        "{e:?}"
    );
}

/// `KIT` with `Keyboard`, plus a component taking `props`,
/// returning `<Screen>{body}</Screen>` on line 2.
fn typing_screen(body: &str) -> String {
    let kit = KIT.replace(" } from", ", Keyboard } from");
    format!("{kit}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n")
}

#[test]
fn a_keyboard_reaches_the_ir() {
    // One, in a screen with a text input, even one a group holds.
    assert_eq!(
        ok_outline(&typing_screen(
            "<Column>{props.ready && <TextInput id=\"name\" />}<Keyboard /></Column>"
        )),
        "column gap=0 padding=0\n  group 1\n    input field=1 max_len=64 mode=text\n  keyboard\n"
    );
}

#[test]
fn a_second_keyboard_or_one_with_no_text_input_is_refused_where_written() {
    for (body, code, marker) in [
        (
            "<Column><TextInput id=\"name\" /><Keyboard /><Keyboard id=\"second\" /></Column>",
            Code::Limit,
            "<Keyboard id",
        ),
        (
            "<Column><Text>Hello</Text><Keyboard /></Column>",
            Code::InvalidChildren,
            "<Keyboard",
        ),
        (
            "<Column><TextInput id=\"name\" /><Keyboard><Text>A</Text></Keyboard></Column>",
            Code::InvalidChildren,
            "<Text>A",
        ),
        (
            "<Column><TextInput id=\"name\" /><Keyboard layout=\"digits\" /></Column>",
            Code::UnknownProp,
            "layout",
        ),
    ] {
        let e = error(&typing_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
}

/// `KIT` with `FieldError`, plus a component taking `props`,
/// returning `<Screen>{body}</Screen>` on line 2.
fn checking_screen(body: &str) -> String {
    let kit = KIT.replace(" } from", ", FieldError } from");
    format!("{kit}export function S(props: P) {{ return <Screen>{body}</Screen>; }}\n")
}

#[test]
fn a_field_error_reaches_the_ir_bound_to_its_control() {
    // Before or after its control, on any control with a field.
    assert_eq!(
        ok_outline(&checking_screen(
            "<Column><FieldError for=\"phone\" /><TextInput id=\"phone\" /><Checkbox id=\"agree\" label=\"Yes\" /><FieldError for=\"agree\" /></Column>"
        )),
        "column gap=0 padding=0\n  field_error field=1\n  input field=1 max_len=64 mode=text\n  checkbox field=2 \"Yes\"\n  field_error field=2\n"
    );
}

#[test]
fn a_field_error_for_no_control_or_a_second_for_one_field_is_refused_where_written() {
    for (body, code, marker) in [
        (
            "<Column><TextInput id=\"phone\" /><FieldError for=\"email\" /></Column>",
            Code::UnknownName,
            "<FieldError",
        ),
        (
            "<Column><Text id=\"title\">Hi</Text><FieldError for=\"title\" /></Column>",
            Code::UnknownName,
            "<FieldError",
        ),
        (
            "<Column><TextInput id=\"phone\" /><FieldError for=\"phone\" /><FieldError id=\"again\" for=\"phone\" /></Column>",
            Code::DuplicateName,
            "<FieldError id",
        ),
        (
            "<Column><TextInput id=\"phone\" /><FieldError /></Column>",
            Code::MissingProp,
            "<FieldError",
        ),
        (
            "<Column><TextInput id=\"phone\" /><FieldError for={props.which} /></Column>",
            Code::InvalidValue,
            "for=",
        ),
        (
            "<Column><TextInput id=\"phone\" /><FieldError for=\"phone\" message=\"Wrong\" /></Column>",
            Code::UnknownProp,
            "message",
        ),
    ] {
        let e = error(&checking_screen(body));
        assert_eq!(
            (e.code, e.loc),
            (code, at(2, column_of(body, marker))),
            "{body}: {e:?}"
        );
    }
}

// SPDX-License-Identifier: MIT
//! Property: any screen built from the subset converts, and its IR is exactly
//! the screen that was written - subset v0's elements, and subset v1's slots
//! and conditional rendering, checkboxes, radio groups and text
//! inputs' length limits and modes.
//!
//! Random trees of kit elements are printed as TSX, run through the whole
//! pipeline, and the IR outline is compared with an outline computed directly
//! from the random tree. Generation must succeed, and generating again from
//! the same IR must give the same text (the command-line tests check that
//! separate runs of the `screen-compiler` binary agree).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

mod support;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};

const CALLBACKS: [&str; 3] = ["onAlpha", "onBeta", "onGamma"];
/// The props conditions test: groups may share one.
const CONDITIONS: [&str; 2] = ["ready", "late"];
/// `inputMode` values, or none, and the mode each is in an outline.
const MODES: [(Option<&str>, &str); 5] = [
    (None, "text"),
    (Some("text"), "text"),
    (Some("numeric"), "digits"),
    (Some("tel"), "digits"),
    (Some("email"), "email"),
];

#[derive(Clone, Copy, Debug)]
enum Len {
    Px(u16),
    Token,
}

#[derive(Clone, Debug)]
enum Gen {
    Column(Option<Len>, Option<Len>, Vec<Gen>),
    Row(Option<Len>, Option<Len>, Vec<Gen>),
    Text(String),
    Button(String, usize),
    /// A placeholder, a `maxLength` and an `inputMode` (an index into
    /// `MODES`).
    TextInput(Option<String>, Option<u16>, usize),
    Spacer(Option<Len>),
    Checkbox(String),
    RadioGroup(Vec<String>),
    /// `<Text>{valueN}</Text>`, each with a prop of its own.
    Slot,
    /// `{cond && <X/>}`.
    Group(usize, Box<Gen>),
    /// `{cond ? <X/> : <Y/>}`.
    Either(usize, Box<Gen>, Box<Gen>),
    /// `{cond ? <X/> : null}`.
    OrNull(usize, Box<Gen>),
}

/// Text that JSX renders exactly as written on a single line.
fn plain_text() -> impl Strategy<Value = String> {
    "[A-Za-z0-9]([A-Za-z0-9 ,.!?'-]{0,14}[A-Za-z0-9.!?])?"
}

fn len() -> impl Strategy<Value = Option<Len>> {
    prop_oneof![
        Just(None),
        any::<u16>().prop_map(|px| Some(Len::Px(px))),
        Just(Some(Len::Token)),
    ]
}

fn tree() -> impl Strategy<Value = Gen> {
    let leaf = prop_oneof![
        plain_text().prop_map(Gen::Text),
        (plain_text(), 0..CALLBACKS.len()).prop_map(|(label, cb)| Gen::Button(label, cb)),
        (
            proptest::option::of(plain_text()),
            proptest::option::of(1u16..=64),
            0..MODES.len()
        )
            .prop_map(|(p, max, mode)| Gen::TextInput(p, max, mode)),
        len().prop_map(Gen::Spacer),
        Just(Gen::Slot),
        plain_text().prop_map(Gen::Checkbox),
        prop::collection::vec(plain_text(), 1..=8).prop_map(Gen::RadioGroup),
    ];
    leaf.prop_recursive(4, 32, 4, |inner| {
        let condition = 0..CONDITIONS.len();
        // What a condition shows is one element.
        let element = inner.clone().prop_map(as_element);
        prop_oneof![
            (len(), len(), prop::collection::vec(inner.clone(), 0..4))
                .prop_map(|(g, p, c)| Gen::Column(g, p, c)),
            (len(), len(), prop::collection::vec(inner.clone(), 0..4))
                .prop_map(|(g, p, c)| Gen::Row(g, p, c)),
            (condition.clone(), element.clone()).prop_map(|(c, x)| Gen::Group(c, Box::new(x))),
            (condition.clone(), element.clone(), element.clone())
                .prop_map(|(c, x, y)| Gen::Either(c, Box::new(x), Box::new(y))),
            (condition, element).prop_map(|(c, x)| Gen::OrNull(c, Box::new(x))),
        ]
    })
}

/// `node`, or a conditional wrapped in a `<Column>`: where JSX needs an
/// element - a screen's root, and what a condition shows.
fn as_element(node: Gen) -> Gen {
    match node {
        Gen::Group(..) | Gen::Either(..) | Gen::OrNull(..) => Gen::Column(None, None, vec![node]),
        other => other,
    }
}

struct Printer {
    tsx: String,
    fields: usize,
    slots: usize,
}

impl Printer {
    fn len(len: Len) -> String {
        match len {
            Len::Px(px) => format!("{{{px}}}"),
            Len::Token => "{SPACE}".to_owned(),
        }
    }

    fn node(&mut self, node: &Gen, indent: usize) {
        let pad = "  ".repeat(indent);
        match node {
            Gen::Column(gap, padding, children) | Gen::Row(gap, padding, children) => {
                let tag = if matches!(node, Gen::Column(..)) {
                    "Column"
                } else {
                    "Row"
                };
                let mut open = format!("{pad}<{tag}");
                if let Some(gap) = gap {
                    open.push_str(&format!(" gap={}", Self::len(*gap)));
                }
                if let Some(padding) = padding {
                    open.push_str(&format!(" padding={}", Self::len(*padding)));
                }
                if children.is_empty() {
                    self.tsx.push_str(&format!("{open} />\n"));
                } else {
                    self.tsx.push_str(&format!("{open}>\n"));
                    for child in children {
                        self.node(child, indent + 1);
                    }
                    self.tsx.push_str(&format!("{pad}</{tag}>\n"));
                }
            }
            Gen::Text(text) => self.tsx.push_str(&format!("{pad}<Text>{text}</Text>\n")),
            Gen::Button(label, cb) => self.tsx.push_str(&format!(
                "{pad}<Button onPress={{{}}}>{label}</Button>\n",
                CALLBACKS[*cb]
            )),
            Gen::TextInput(placeholder, max_len, mode) => {
                self.fields += 1;
                let mut props = placeholder
                    .as_ref()
                    .map(|p| format!(" placeholder=\"{p}\""))
                    .unwrap_or_default();
                if let Some(max_len) = max_len {
                    props.push_str(&format!(" maxLength={{{max_len}}}"));
                }
                if let Some(mode) = MODES[*mode].0 {
                    props.push_str(&format!(" inputMode=\"{mode}\""));
                }
                self.tsx.push_str(&format!(
                    "{pad}<TextInput id=\"field{}\"{props} />\n",
                    self.fields
                ));
            }
            Gen::Checkbox(label) => {
                self.fields += 1;
                self.tsx.push_str(&format!(
                    "{pad}<Checkbox id=\"field{}\" label=\"{label}\" />\n",
                    self.fields
                ));
            }
            Gen::RadioGroup(options) => {
                self.fields += 1;
                let options: Vec<String> = options.iter().map(|o| format!("\"{o}\"")).collect();
                self.tsx.push_str(&format!(
                    "{pad}<RadioGroup id=\"field{}\" options={{[{}]}} />\n",
                    self.fields,
                    options.join(", ")
                ));
            }
            Gen::Spacer(None) => self.tsx.push_str(&format!("{pad}<Spacer />\n")),
            Gen::Spacer(Some(size)) => self
                .tsx
                .push_str(&format!("{pad}<Spacer size={} />\n", Self::len(*size))),
            Gen::Slot => {
                self.slots += 1;
                self.tsx
                    .push_str(&format!("{pad}<Text>{{value{}}}</Text>\n", self.slots));
            }
            Gen::Group(c, x) => {
                self.tsx
                    .push_str(&format!("{pad}{{{} && (\n", CONDITIONS[*c]));
                self.node(x, indent + 1);
                self.tsx.push_str(&format!("{pad})}}\n"));
            }
            Gen::Either(c, x, y) => {
                self.tsx
                    .push_str(&format!("{pad}{{{} ? (\n", CONDITIONS[*c]));
                self.node(x, indent + 1);
                self.tsx.push_str(&format!("{pad}) : (\n"));
                self.node(y, indent + 1);
                self.tsx.push_str(&format!("{pad})}}\n"));
            }
            Gen::OrNull(c, x) => {
                self.tsx
                    .push_str(&format!("{pad}{{{} ? (\n", CONDITIONS[*c]));
                self.node(x, indent + 1);
                self.tsx.push_str(&format!("{pad}) : null}}\n"));
            }
        }
    }
}

fn source(root: &Gen, space: u16) -> String {
    let mut printer = Printer {
        tsx: String::new(),
        fields: 0,
        slots: 0,
    };
    // The root is an element: a screen's one child cannot be a condition.
    printer.node(&as_element(root.clone()), 3);
    let values: String = (1..=printer.slots).map(|n| format!(", value{n}")).collect();
    format!(
        "import {{ Button, Checkbox, Column, RadioGroup, Row, Screen, Spacer, Text, TextInput }} from \"nowayos:kit\";\n\nconst SPACE = {space};\n\nexport function Generated({{ onAlpha, onBeta, onGamma, ready, late{values} }}: Props) {{\n  return (\n    <Screen>\n{}    </Screen>\n  );\n}}\n",
        printer.tsx
    )
}

/// The outline `screen_compiler_ir::outline` must produce for `root`.
fn expected_outline(root: &Gen, space: u16) -> String {
    fn px(len: Option<Len>, space: u16) -> u16 {
        match len {
            None => 0,
            Some(Len::Px(px)) => px,
            Some(Len::Token) => space,
        }
    }
    /// How generated code numbers actions, groups and slots.
    #[derive(Default)]
    struct Numbers {
        actions: Vec<usize>,
        groups: Vec<usize>,
        slots: usize,
        fields: usize,
    }
    fn first_seen(seen: &mut Vec<usize>, item: usize) -> usize {
        match seen.iter().position(|a| *a == item) {
            Some(i) => i + 1,
            None => {
                seen.push(item);
                seen.len()
            }
        }
    }
    fn walk(node: &Gen, depth: usize, space: u16, numbers: &mut Numbers, out: &mut String) {
        out.push_str(&"  ".repeat(depth));
        match node {
            Gen::Column(gap, padding, children) | Gen::Row(gap, padding, children) => {
                let tag = if matches!(node, Gen::Column(..)) {
                    "column"
                } else {
                    "row"
                };
                out.push_str(&format!(
                    "{tag} gap={} padding={}\n",
                    px(*gap, space),
                    px(*padding, space)
                ));
                for child in children {
                    walk(child, depth + 1, space, numbers, out);
                }
            }
            Gen::Text(text) => out.push_str(&format!("text {text:?}\n")),
            Gen::Button(label, cb) => {
                let number = first_seen(&mut numbers.actions, *cb);
                out.push_str(&format!("button {label:?} action={number}\n"));
            }
            Gen::Slot => {
                numbers.slots += 1;
                out.push_str(&format!("slot {}\n", numbers.slots));
            }
            Gen::Group(c, x) | Gen::OrNull(c, x) => {
                let number = first_seen(&mut numbers.groups, *c);
                out.push_str(&format!("group {number}\n"));
                walk(x, depth + 1, space, numbers, out);
            }
            Gen::Either(c, x, y) => {
                let number = first_seen(&mut numbers.groups, *c);
                out.push_str(&format!("group {number}\n"));
                walk(x, depth + 1, space, numbers, out);
                walk(y, depth + 1, space, numbers, out);
            }
            Gen::TextInput(placeholder, max_len, mode) => {
                numbers.fields += 1;
                out.push_str(&format!(
                    "input field={} max_len={} mode={}",
                    numbers.fields,
                    max_len.unwrap_or(64),
                    MODES[*mode].1
                ));
                if let Some(p) = placeholder {
                    out.push_str(&format!(" placeholder={p:?}"));
                }
                out.push('\n');
            }
            Gen::Checkbox(label) => {
                numbers.fields += 1;
                out.push_str(&format!("checkbox field={} {label:?}\n", numbers.fields));
            }
            Gen::RadioGroup(options) => {
                numbers.fields += 1;
                out.push_str(&format!(
                    "radio_group field={} {options:?}\n",
                    numbers.fields
                ));
            }
            Gen::Spacer(None) => out.push_str("spacer\n"),
            Gen::Spacer(Some(size)) => {
                out.push_str(&format!("spacer size={}\n", px(Some(*size), space)));
            }
        }
    }
    let mut out = String::new();
    walk(
        &as_element(root.clone()),
        0,
        space,
        &mut Numbers::default(),
        &mut out,
    );
    out
}

fn config() -> Config {
    Config {
        cases: 128,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(0x7363_7265_656e_7331),
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn any_subset_screen_converts_to_exactly_itself(root in tree(), space in any::<u16>()) {
        let tsx = source(&root, space);
        let (module, rust) = support::convert_source("generated.tsx", &tsx);
        prop_assert_eq!(
            screen_compiler_ir::outline(&module, &module.screens[0]),
            expected_outline(&root, space),
            "source:\n{}", tsx
        );
        prop_assert_eq!(screen_compiler_codegen_rust::generate(&module).unwrap(), rust);
    }
}

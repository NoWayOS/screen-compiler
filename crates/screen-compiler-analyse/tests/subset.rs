// SPDX-License-Identifier: MIT
//! The subset as data (`screen-compiler subset`) says exactly what
//! converts: `kit/subset.json` is what the tables print, and the analyser
//! agrees with the tables - each component converts with its required
//! props and not without any of them, every value the tables list for a
//! prop or a style property converts, values outside them are refused, and
//! so is a prop or a style property they do not list.
//!
//! Set `SCREEN_COMPILER_BLESS=1` to rewrite `kit/subset.json` after a
//! deliberate change (`make bless`), then review the diff.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use screen_compiler_analyse::{
    Children, KitComponent, Options, Prop, StyleProperty, Value, analyse, subset,
};
use screen_compiler_ir::Colour;

#[test]
fn the_committed_subset_is_what_the_tables_print() {
    let json = subset::json();
    let committed = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../kit/subset.json");
    if std::env::var_os("SCREEN_COMPILER_BLESS").is_some() {
        fs::write(&committed, &json).unwrap();
        return;
    }
    let expected = fs::read_to_string(&committed).unwrap_or_default();
    assert!(
        expected == json,
        "the kit's tables changed; if on purpose, run `make bless` and review kit/subset.json's diff"
    );
}

#[test]
fn every_component_converts_with_its_required_props_and_not_without_each() {
    for component in KitComponent::ALL {
        converts(&screen(component, &Change::None))
            .unwrap_or_else(|e| panic!("<{component}>: {e}"));
        for prop in component.props().iter().filter(|p| p.required) {
            assert!(
                converts(&screen(component, &Change::Without(prop.name))).is_err(),
                "<{component}> converts without {}",
                prop.name
            );
        }
    }
}

#[test]
fn every_value_listed_for_a_prop_converts() {
    for component in KitComponent::ALL {
        for prop in component.props() {
            for value in valid(prop.value) {
                let change = Change::Prop(prop.name, attribute(&value));
                converts(&screen(component, &change))
                    .unwrap_or_else(|e| panic!("<{component} {}={value}>: {e}", prop.name));
            }
        }
    }
}

#[test]
fn every_value_listed_for_a_style_property_converts() {
    for component in KitComponent::ALL {
        for property in component.style() {
            for value in valid(property.value) {
                let change = Change::Style(property.name, value.clone());
                converts(&screen(component, &change)).unwrap_or_else(|e| {
                    panic!(
                        "<{component} style={{{{ {}: {value} }}}}>: {e}",
                        property.name
                    )
                });
            }
        }
    }
}

#[test]
fn values_outside_the_lists_are_refused() {
    for component in KitComponent::ALL {
        for &Prop { name, value, .. } in component.props() {
            for wrong in invalid(value) {
                let change = Change::Prop(name, attribute(&wrong));
                assert!(
                    converts(&screen(component, &change)).is_err(),
                    "<{component} {name}={wrong}> converts"
                );
            }
        }
        for &StyleProperty { name, value } in component.style() {
            for wrong in invalid(value) {
                let change = Change::Style(name, wrong.clone());
                assert!(
                    converts(&screen(component, &change)).is_err(),
                    "<{component} style={{{{ {name}: {wrong} }}}}> converts"
                );
            }
        }
    }
}

#[test]
fn a_prop_or_a_style_property_the_tables_do_not_list_is_refused() {
    let all_props: BTreeSet<&str> = KitComponent::ALL
        .iter()
        .flat_map(|c| c.props().iter().map(|p| p.name))
        .collect();
    let all_style: BTreeSet<&str> = KitComponent::ALL
        .iter()
        .flat_map(|c| c.style().iter().map(|p| p.name))
        .collect();
    for component in KitComponent::ALL {
        for name in &all_props {
            if component.prop(name).is_none() {
                let change = Change::Prop(name, "\"x\"".to_owned());
                assert!(
                    converts(&screen(component, &change)).is_err(),
                    "<{component} {name}> converts"
                );
            }
        }
        if component.style().is_empty() {
            continue;
        }
        for name in &all_style {
            if !component.styles(name) {
                let change = Change::Style(name, "\"text\"".to_owned());
                assert!(
                    converts(&screen(component, &change)).is_err(),
                    "<{component} style={{{{ {name} }}}}> converts"
                );
            }
        }
    }
}

/// What a test changes from a component's least screen.
enum Change {
    None,
    /// Leave out a required prop.
    Without(&'static str),
    /// Give a prop this value, as written after `=`.
    Prop(&'static str, String),
    /// Give a style property this value.
    Style(&'static str, String),
}

/// A value as written after a prop's `=`: a string literal as it is,
/// anything else in braces.
fn attribute(value: &str) -> String {
    if value.starts_with('"') {
        value.to_owned()
    } else {
        format!("{{{value}}}")
    }
}

/// Values of each kind that convert, as written in an expression.
fn valid(value: Value) -> Vec<String> {
    match value {
        Value::Id => vec!["\"subject\"".into()],
        Value::FieldId => vec!["\"field\"".into()],
        Value::Text => vec!["\"Sample\"".into()],
        Value::Length => vec!["8".into(), "TOKEN".into(), "space.md".into()],
        Value::Size => vec!["120".into(), "TOKEN".into()],
        Value::Whole { least, most, token } => {
            let mut values = vec![least.to_string(), most.to_string()];
            if token {
                values.push("TOKEN".into());
            }
            values
        }
        Value::Choice(names) => names.iter().map(|n| format!("\"{n}\"")).collect(),
        Value::Colour => Colour::NAMES.iter().map(|n| format!("\"{n}\"")).collect(),
        Value::Action => vec!["props.onGo".into()],
        Value::Prop => vec!["props.done".into()],
        Value::Options { most } => vec![options(1), options(most)],
        Value::Style => vec!["{}".into()],
    }
}

/// Values of each kind that do not convert.
fn invalid(value: Value) -> Vec<String> {
    match value {
        Value::Id => vec!["\"1st\"".into(), "props.done".into()],
        Value::FieldId => vec!["\"nowhere\"".into()],
        Value::Text => vec!["props.done".into()],
        Value::Length => vec!["\"8px\"".into(), "props.done".into()],
        Value::Size => vec!["space.md".into(), "\"120px\"".into()],
        Value::Whole { least, most, .. } => {
            let mut values = vec![(most + 1).to_string(), "\"1\"".into()];
            if least > 0 {
                values.push((least - 1).to_string());
            }
            values
        }
        Value::Choice(_) => vec!["\"not-a-value\"".into()],
        Value::Colour => vec!["\"#ffffff\"".into(), "\"red\"".into()],
        Value::Action => vec!["\"go\"".into(), "TOKEN".into()],
        Value::Prop => vec!["5".into()],
        Value::Options { most } => vec![options(most + 1), "[]".into(), "props.done".into()],
        Value::Style => vec!["\"gap: 8\"".into()],
    }
}

/// An array of `n` string literals.
fn options(n: usize) -> String {
    let items: Vec<String> = (1..=n).map(|i| format!("\"Option {i}\"")).collect();
    format!("[{}]", items.join(", "))
}

/// The least screen holding `component`, as `change` alters it: beside a
/// text input with the id `field`, which a field error may name and a
/// keyboard needs.
fn screen(component: KitComponent, change: &Change) -> String {
    let mut kit: BTreeSet<&str> = ["Screen", "Column", "Text", "TextInput", "space"].into();
    kit.insert(component.name());
    let root = if component == KitComponent::Screen {
        element(component, change)
    } else {
        format!(
            "<Screen>\n      <Column>\n        <TextInput id=\"field\" />\n        {}\n      </Column>\n    </Screen>",
            element(component, change)
        )
    };
    let imports: Vec<&str> = kit.into_iter().collect();
    format!(
        "import {{ {} }} from \"nowayos:kit\";\n\n\
         const TOKEN = 12;\n\n\
         export function Sample(props) {{\n  return (\n    {root}\n  );\n}}\n",
        imports.join(", ")
    )
}

fn element(component: KitComponent, change: &Change) -> String {
    let mut attrs = Vec::new();
    for prop in component.props() {
        let given = match change {
            Change::Without(name) if *name == prop.name => None,
            Change::Prop(name, value) if *name == prop.name => Some(value.clone()),
            _ if prop.required => Some(attribute(&valid(prop.value)[0])),
            _ => None,
        };
        if let Some(value) = given {
            attrs.push(format!("{}={value}", prop.name));
        }
    }
    match change {
        Change::Prop(name, value) if component.prop(name).is_none() => {
            attrs.push(format!("{name}={value}"));
        }
        Change::Style(name, value) => attrs.push(format!("style={{{{ {name}: {value} }}}}")),
        _ => {}
    }
    let name = component.name();
    let open = std::iter::once(name.to_owned())
        .chain(attrs)
        .collect::<Vec<_>>()
        .join(" ");
    match component.children() {
        Children::None => format!("<{open} />"),
        Children::One => format!("<{open}><Column><Text>SAMPLE</Text></Column></{name}>"),
        Children::Elements => format!("<{open}><Text>SAMPLE</Text></{name}>"),
        Children::TextOrProp | Children::Text => format!("<{open}>SAMPLE</{name}>"),
    }
}

/// Whether `source` converts; otherwise its diagnostics, with the source.
fn converts(source: &str) -> Result<(), String> {
    let diagnostics = match screen_compiler_tsx::parse(source) {
        Ok(parsed) => {
            let analysis = analyse(&parsed, &Options::new("sample.tsx"));
            if analysis.module.is_some() && !analysis.diagnostics.iter().any(|d| d.is_error()) {
                return Ok(());
            }
            analysis.diagnostics
        }
        Err(diagnostics) => diagnostics,
    };
    let rendered: Vec<String> = diagnostics.iter().map(|d| d.render("sample.tsx")).collect();
    Err(format!("{}\n{source}", rendered.join("\n")))
}

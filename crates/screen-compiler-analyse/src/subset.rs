// SPDX-License-Identifier: MIT
//! The subset as data (`screen-compiler subset`): every component of the
//! kit, the props and style properties it takes and the values each
//! accepts, the theme's colours and its spacing tokens - printed from the
//! tables the analyser checks against ([`crate::KitComponent`]), so help written
//! from it cannot promise what does not convert, nor leave out what does.
//!
//! The output is JSON, deterministic, pure ASCII and one prop to a line:
//!
//! ```text
//! {
//!   "module": "nowayos:kit",
//!   "components": [
//!     {
//!       "name": "Column",
//!       "children": "elements",
//!       "props": [
//!         { "name": "gap", "required": false, "value": { "kind": "length" } },
//!         …
//!       ],
//!       "style": [
//!         { "name": "alignItems", "value": { "kind": "choice", "values": ["flex-start", …] } },
//!         …
//!       ]
//!     },
//!     …
//!   ],
//!   "colours": ["background", …],
//!   "space": ["xs", …]
//! }
//! ```
//!
//! A value's `kind` is `id`, `field-id`, `text`, `length`, `size`,
//! `whole` (with `least`, `most` and `token`), `choice` (with `values`),
//! `colour`, `action`, `prop`, `options` (with `most`) or `style`; a
//! component's `children` is `none`, `one-element`, `elements`,
//! `text-or-prop` or `text` ([`Children`](crate::Children)).

use std::fmt::Write as _;

use screen_compiler_ir::{Colour, Space};

use crate::KIT_MODULE;
use crate::kit::{KitComponent, Value};

/// The subset as JSON, ending in a newline.
#[must_use]
pub fn json() -> String {
    let mut out = String::from("{\n");
    let _ = writeln!(out, "  \"module\": {},", string(KIT_MODULE));
    out.push_str("  \"components\": [\n");
    for (i, component) in KitComponent::ALL.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        component_json(&mut out, *component);
    }
    out.push_str("\n  ],\n");
    let _ = writeln!(out, "  \"colours\": {},", strings(&Colour::NAMES));
    let _ = writeln!(out, "  \"space\": {}", strings(&Space::NAMES));
    out.push_str("}\n");
    out
}

fn component_json(out: &mut String, component: KitComponent) {
    out.push_str("    {\n");
    let _ = writeln!(out, "      \"name\": {},", string(component.name()));
    let _ = writeln!(
        out,
        "      \"children\": {},",
        string(component.children().name())
    );
    let props: Vec<String> = component
        .props()
        .iter()
        .map(|p| {
            format!(
                "{{ \"name\": {}, \"required\": {}, \"value\": {} }}",
                string(p.name),
                p.required,
                value_json(p.value)
            )
        })
        .collect();
    list(out, "props", &props, true);
    let style: Vec<String> = component
        .style()
        .iter()
        .map(|p| {
            format!(
                "{{ \"name\": {}, \"value\": {} }}",
                string(p.name),
                value_json(p.value)
            )
        })
        .collect();
    list(out, "style", &style, false);
    out.push_str("    }");
}

/// `"key": [ … ]`, one item to a line; `[]` when empty.
fn list(out: &mut String, key: &str, items: &[String], more: bool) {
    let comma = if more { "," } else { "" };
    if items.is_empty() {
        let _ = writeln!(out, "      \"{key}\": []{comma}");
        return;
    }
    let _ = writeln!(out, "      \"{key}\": [");
    let _ = writeln!(out, "        {}", items.join(",\n        "));
    let _ = writeln!(out, "      ]{comma}");
}

fn value_json(value: Value) -> String {
    let kind = |kind: &str| format!("{{ \"kind\": {} }}", string(kind));
    match value {
        Value::Id => kind("id"),
        Value::FieldId => kind("field-id"),
        Value::Text => kind("text"),
        Value::Length => kind("length"),
        Value::Size => kind("size"),
        Value::Whole { least, most, token } => format!(
            "{{ \"kind\": \"whole\", \"least\": {least}, \"most\": {most}, \"token\": {token} }}"
        ),
        Value::Choice(values) => format!(
            "{{ \"kind\": \"choice\", \"values\": {} }}",
            strings(values)
        ),
        Value::Colour => kind("colour"),
        Value::Action => kind("action"),
        Value::Prop => kind("prop"),
        Value::Options { most } => format!("{{ \"kind\": \"options\", \"most\": {most} }}"),
        Value::Style => kind("style"),
    }
}

fn strings(values: &[&str]) -> String {
    let quoted: Vec<String> = values.iter().map(|v| string(v)).collect();
    format!("[{}]", quoted.join(", "))
}

/// A JSON string: `"` and `\` escaped, and anything outside printable
/// ASCII as `\uXXXX` (the tables hold none, but the output stays ASCII
/// whatever they hold).
fn string(text: &str) -> String {
    let mut out = String::from("\"");
    for unit in text.encode_utf16() {
        match unit {
            0x22 => out.push_str("\\\""),
            0x5C => out.push_str("\\\\"),
            0x20..=0x7E => out.push(char::from(u8::try_from(unit).unwrap_or(b'?'))),
            _ => {
                let _ = write!(out, "\\u{unit:04x}");
            }
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped_and_ascii() {
        assert_eq!(string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(string("\u{a3}\n"), "\"\\u00a3\\u000a\"");
        assert_eq!(string("\u{1F600}"), "\"\\ud83d\\ude00\"");
    }

    #[test]
    fn the_output_is_ascii_and_names_every_component() {
        let json = json();
        assert!(json.is_ascii());
        for component in KitComponent::ALL {
            assert!(
                json.contains(&format!("\"name\": \"{component}\"")),
                "{component}"
            );
        }
    }
}

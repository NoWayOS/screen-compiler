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
//! `text-or-prop` or `text` ([`Children`]).
//!
//! [`declarations`] writes the same tables as TypeScript declarations of
//! `nowayos:kit` (`screen-compiler subset --dts`), so an editor that bundles
//! TypeScript flags an unknown component, an unknown prop or a value
//! outside the lists as it is typed. They declare the global `JSX`
//! namespace themselves, for a project with no React types (`"jsx":
//! "preserve"`): no HTML elements, no `key`, and an element's type branded
//! so that text cannot stand where an element must.

use std::fmt::Write as _;

use screen_compiler_ir::{Colour, Space};

use crate::KIT_MODULE;
use crate::kit::{Children, KitComponent, Prop, StyleProperty, Value};

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
    let inline: Vec<String> = crate::kit::INLINE
        .iter()
        .map(|(name, props)| {
            format!(
                "{{ \"name\": {}, \"props\": {} }}",
                string(name),
                strings(props)
            )
        })
        .collect();
    let _ = writeln!(out, "  \"inline\": [{}],", inline.join(", "));
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
            let or =
                p.or.map(|or| format!(", \"or\": {}", string(or)))
                    .unwrap_or_default();
            format!(
                "{{ \"name\": {}, \"required\": {}{or}, \"value\": {} }}",
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

/// A range of whole numbers no wider than this is declared as a union of
/// its literals (`1 | 2 | … | 8`); a wider one as `number`.
const MOST_LITERALS: u16 = 17;

/// The subset as TypeScript declarations of `nowayos:kit`, ending in a
/// newline.
#[must_use]
pub fn declarations() -> String {
    let mut out = String::from(
        "// The declarations of `nowayos:kit`, written by `screen-compiler subset --dts` from\n\
         // Screen Compiler's own tables: do not edit. For an editor's TypeScript in a folder\n\
         // of screens without React's types: \"jsx\": \"preserve\" in its tsconfig.json.\n\
         // Screen Compiler checks more than these types can say (a text's characters, an\n\
         // id's form, where a field error or a keyboard may go): it has the last word.\n\n",
    );
    out.push_str(
        "declare namespace JSX {\n\
         \x20 /** An element of a screen: one of the kit's components. */\n\
         \x20 interface Element {\n\
         \x20   readonly \"nowayos:kit\": true;\n\
         \x20 }\n\
         \x20 /** Only inside a <Text>, around its words (contract v21): a rich text. */\n\
         \x20 interface IntrinsicElements {\n\
         \x20   strong: { children?: unknown };\n\
         \x20   b: { children?: unknown };\n\
         \x20   em: { children?: unknown };\n\
         \x20   i: { children?: unknown };\n\
         \x20   code: { children?: unknown };\n\
         \x20   a: { onClick?: () => void; onPress?: () => void; children?: unknown };\n\
         \x20   span: { style: { color: import(\"nowayos:kit\").Colour }; children?: unknown };\n\
         \x20 }\n\
         \x20 interface ElementChildrenAttribute {\n\
         \x20   children: {};\n\
         \x20 }\n\
         }\n\n",
    );
    let _ = writeln!(out, "declare module {} {{", string(KIT_MODULE));
    let _ = writeln!(
        out,
        "  /** One of the theme's colours, by name: never a colour value. */\n  export type Colour = {};\n",
        union(&Colour::NAMES)
    );
    let _ = writeln!(
        out,
        "  /** One of the theme's spacing tokens, which grow with the large-text theme. */\n  \
         export interface Space {{\n    readonly \"nowayos:space\": {};\n  }}\n",
        union(&Space::NAMES)
    );
    let tokens: Vec<String> = Space::NAMES
        .iter()
        .map(|name| format!("readonly {name}: Space"))
        .collect();
    let _ = writeln!(
        out,
        "  /** The theme's spacing tokens: `space.xs` to `space.xl`. */\n  export const space: {{ {} }};\n",
        tokens.join("; ")
    );
    out.push_str(
        "  /**\n   \
         * What a column or a row holds: elements, and the parts the application shows or hides\n   \
         * (`{props.ready && <X />}` is the prop's falsy value while it is hidden).\n   \
         */\n  \
         export type Child = JSX.Element | false | null | undefined | \"\" | 0;\n",
    );
    for component in KitComponent::ALL {
        component_declarations(&mut out, component);
    }
    out.push_str("}\n");
    out
}

fn component_declarations(out: &mut String, component: KitComponent) {
    let name = component.name();
    if !component.style().is_empty() {
        let _ = writeln!(out, "\n  /** What `style` on `<{name}>` may name. */");
        let _ = writeln!(out, "  export interface {name}Style {{");
        for &StyleProperty {
            name: property,
            value,
        } in component.style()
        {
            member(out, property, false, value, name);
        }
        out.push_str("  }\n");
    }
    let _ = writeln!(out, "\n  /** The props of `<{name}>`. */");
    // A pair it takes one of, never both, is a union of the two.
    let pair: Vec<&Prop> = component
        .props()
        .iter()
        .filter(|p| p.or.is_some())
        .collect();
    if pair.is_empty() {
        let _ = writeln!(out, "  export interface {name}Props {{");
    } else {
        let _ = writeln!(out, "  export type {name}Props = {name}OneOf & {{");
    }
    for prop in component.props().iter().filter(|p| p.or.is_none()) {
        member(out, prop.name, prop.required, prop.value, name);
    }
    let children = match component.children() {
        Children::None => "  children?: never;",
        Children::One => "  /** Exactly one element. */\n    children: JSX.Element;",
        Children::Elements => "  children?: Child | readonly Child[];",
        Children::TextOrProp => {
            "  /** Literal text, or one prop the application sets. */\n    children: string;"
        }
        Children::Text => "  /** Its label: literal text. */\n    children: string;",
    };
    let _ = writeln!(out, "  {children}");
    if pair.is_empty() {
        out.push_str("  }\n");
    } else {
        out.push_str("  };\n");
        let _ = writeln!(
            out,
            "  /** `<{name}>` takes one of {}, never both. */",
            pair.iter()
                .map(|p| format!("`{}`", p.name))
                .collect::<Vec<_>>()
                .join(" and ")
        );
        let _ = writeln!(out, "  export type {name}OneOf =");
        for prop in &pair {
            let _ = writeln!(out, "    | {{");
            let mut one = String::new();
            member(&mut one, prop.name, true, prop.value, name);
            for line in one.lines() {
                let _ = writeln!(out, "    {line}");
            }
            if let Some(or) = prop.or {
                let _ = writeln!(out, "        {or}?: never;");
            }
            let last = pair.last().is_some_and(|last| last.name == prop.name);
            let _ = writeln!(out, "      }}{}", if last { ";" } else { "" });
        }
    }
    let _ = writeln!(
        out,
        "  /** {} */\n  export function {name}(props: {name}Props): JSX.Element;",
        component.summary()
    );
}

/// One property of a props or style interface, with what it takes.
fn member(out: &mut String, name: &str, required: bool, value: Value, component: &str) {
    if let Some(doc) = doc(value) {
        let _ = writeln!(out, "    /** {doc} */");
    }
    let optional = if required { "" } else { "?" };
    let _ = writeln!(out, "    {name}{optional}: {};", ts_type(value, component));
}

/// The TypeScript type of a value.
fn ts_type(value: Value, component: &str) -> String {
    match value {
        Value::Id | Value::FieldId | Value::Text => "string".to_owned(),
        Value::Length => "number | `${number}px` | Space".to_owned(),
        Value::Size => "number | `${number}px`".to_owned(),
        Value::Whole {
            least,
            most,
            token: false,
        } if most.saturating_sub(least) < MOST_LITERALS => {
            let literals: Vec<String> = (least..=most).map(|n| n.to_string()).collect();
            literals.join(" | ")
        }
        Value::Whole { .. } => "number".to_owned(),
        Value::Choice(values) => union(values),
        Value::Colour => "Colour | `var(--${Colour})`".to_owned(),
        Value::Action => "() => void".to_owned(),
        Value::Prop => "number".to_owned(),
        Value::Options { .. } => "readonly string[]".to_owned(),
        Value::Style => format!("{component}Style"),
    }
}

/// What a value takes, in words, where its type does not say it all.
fn doc(value: Value) -> Option<String> {
    Some(match value {
        Value::Id => "An id: letters, digits, `-` and `_`, starting with a letter.".to_owned(),
        Value::FieldId => "The id of a field on the same screen.".to_owned(),
        Value::Text => "A string literal.".to_owned(),
        Value::Length => {
            "A whole number of layout units (`16`, or `\"16px\"`), a design token or a spacing token (`space.md`)."
                .to_owned()
        }
        Value::Size => {
            "A whole number of layout units (`16`, or `\"16px\"`) or a design token.".to_owned()
        }
        Value::Whole { least, most, token } => {
            let or = if token { ", or a design token" } else { "" };
            format!("A whole number from {least} to {most}{or}.")
        }
        Value::Action => "A callback prop of the screen, which becomes an action; or a function that only calls one: `() => props.onSave()`.".to_owned(),
        Value::Prop => "A prop of the screen, whose value the application sets.".to_owned(),
        Value::Options { most } => format!("1 to {most} string literals."),
        Value::Colour => {
            "One of the theme's colours, by name or as `var(--name)`.".to_owned()
        }
        Value::Choice(_) | Value::Style => return None,
    })
}

/// `"a" | "b" | "c"`.
fn union(values: &[&str]) -> String {
    let quoted: Vec<String> = values.iter().map(|v| string(v)).collect();
    quoted.join(" | ")
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
    fn the_declarations_are_ascii_and_declare_every_component() {
        let dts = declarations();
        assert!(dts.is_ascii());
        for component in KitComponent::ALL {
            assert!(
                dts.contains(&format!(
                    "export function {component}(props: {component}Props)"
                )),
                "{component}"
            );
        }
        assert_eq!(dts.matches('{').count(), dts.matches('}').count());
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

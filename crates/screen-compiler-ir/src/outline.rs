// SPDX-License-Identifier: MIT
//! A plain-text outline of a screen: one line per node, indented by depth.
//!
//! The outline shows effective values (a screen's own tokens resolved,
//! unset lengths as 0, a theme's spacing token by name, as `space.md`)
//! and numbers actions the way generated code does, so a test can build the
//! generated Rust against a recording builder and compare its outline with
//! this one to prove the two trees are the same.

use crate::{
    ButtonVariant, Container, Length, Look, Module, Name, Node, NodeKind, Screen, TextAlign,
    TextStyle, Weight, validate::MAX_DEPTH,
};

/// Renders `screen` as an indented outline. Lines end with `\n`.
///
/// ```text
/// column gap=8 padding=24
///   text "Welcome"
///   button "Start" action=1
///   spacer
/// ```
#[must_use]
pub fn outline(module: &Module, screen: &Screen) -> String {
    let numbers = Numbers {
        actions: screen.actions(),
        fields: screen.fields(),
        groups: screen.groups(),
        slots: screen.slots(),
    };
    let mut out = String::new();
    let mut stack: Vec<(&Node, usize)> = vec![(&screen.root, 0)];
    while let Some((node, depth)) = stack.pop() {
        for _ in 0..depth {
            out.push_str("  ");
        }
        out.push_str(&describe(module, &numbers, node));
        out.push('\n');
        if depth < MAX_DEPTH {
            let next = depth.saturating_add(1);
            stack.extend(node.children().iter().rev().map(|c| (c, next)));
        }
    }
    out
}

/// How generated code numbers a screen's actions, groups and slots.
struct Numbers<'s> {
    actions: Vec<&'s Name>,
    fields: Vec<&'s Name>,
    groups: Vec<&'s Name>,
    slots: Vec<&'s Name>,
}

/// A container's surface in the outline: nothing for none;
/// otherwise ` surface=SURFACE`.
fn on(surface: Option<crate::Surface>) -> String {
    surface.map_or_else(String::new, |surface| {
        format!(" surface={}", surface.outline())
    })
}

/// A text's style in the outline: nothing for body text, Regular;
/// otherwise ` role=ROLE weight=WEIGHT`; then ` lines=N` if it wraps;
/// then ` align=ALIGN` unless it is set against the start.
fn styled(style: TextStyle) -> String {
    let mut lines = if style.lines == 1 {
        String::new()
    } else {
        format!(" lines={}", style.lines)
    };
    if style.align != TextAlign::Start {
        lines.push_str(&format!(" align={}", style.align.tsx()));
    }
    if let Some(colour) = style.colour {
        lines.push_str(&format!(" colour={}", colour.tsx()));
    }
    let plain = TextStyle {
        lines: 1,
        align: TextAlign::Start,
        colour: None,
        ..style
    };
    if plain == TextStyle::BODY {
        return lines;
    }
    let weight = match style.weight {
        Weight::Regular => "regular",
        Weight::Bold => "bold",
    };
    format!(" role={} weight={weight}{lines}", style.role)
}

/// A container's style in the outline, after its gap, padding and
/// surface: ` width=W height=H min-width=W max-width=W align=A justify=J`
/// then its look, each only where named.
fn styled_container(module: &Module, container: &Container) -> String {
    let mut out = String::new();
    for (name, length) in [
        ("width", &container.width),
        ("height", &container.height),
        ("min-width", &container.min_width),
        ("max-width", &container.max_width),
    ] {
        let units = length.as_ref().map(|l| module.resolve(l).unwrap_or(0));
        // A least width of 0 is none.
        if let Some(units) = units.filter(|u| *u > 0 || name != "min-width") {
            out.push_str(&format!(" {name}={units}"));
        }
    }
    // Each only where it is not the default: stretched, from the start.
    if let Some(align) = container.align.filter(|a| *a != crate::AlignItems::Stretch) {
        out.push_str(&format!(" align={}", align.tsx()));
    }
    if let Some(justify) = container.justify.filter(|j| *j != crate::Justify::Start) {
        out.push_str(&format!(" justify={}", justify.tsx()));
    }
    out.push_str(&looked(container.look));
    out
}

/// A look in the outline: ` fill=C border=C border-width=N
/// radius=N label=C`, each only where named.
fn looked(look: Look) -> String {
    let mut out = String::new();
    if let Some(fill) = look.fill {
        out.push_str(&format!(" fill={}", fill.tsx()));
    }
    if let Some(border) = look.border {
        out.push_str(&format!(" border={}", border.tsx()));
    }
    if let Some(width) = look.border_width {
        out.push_str(&format!(" border-width={width}"));
    }
    if let Some(radius) = look.radius {
        out.push_str(&format!(" radius={radius}"));
    }
    if let Some(text) = look.text {
        out.push_str(&format!(" label={}", text.tsx()));
    }
    out
}

/// `name`'s number in `names`, from 1; 0 if it is not there.
fn number(names: &[&Name], name: &Name) -> usize {
    names
        .iter()
        .position(|n| n.same_words(name))
        .and_then(|i| i.checked_add(1))
        .unwrap_or(0)
}

fn describe(module: &Module, numbers: &Numbers<'_>, node: &Node) -> String {
    // A theme's spacing token by name; any other length in pixels.
    let px = |length: Option<&Length>| match length {
        Some(Length::Space(space)) => format!("space.{}", space.tsx()),
        _ => length
            .and_then(|l| module.resolve(l))
            .unwrap_or(0)
            .to_string(),
    };
    match &node.kind {
        NodeKind::Column(c) => format!(
            "column gap={} padding={}{}{}",
            px(c.gap.as_ref()),
            px(c.padding.as_ref()),
            on(c.surface),
            styled_container(module, c)
        ),
        NodeKind::Row(c) => format!(
            "row gap={} padding={}{}{}",
            px(c.gap.as_ref()),
            px(c.padding.as_ref()),
            on(c.surface),
            styled_container(module, c)
        ),
        NodeKind::Text { text, style } => format!("text {text:?}{}", styled(*style)),
        NodeKind::Button {
            label,
            action,
            variant,
            look,
        } => {
            let variant = if *variant == ButtonVariant::Primary {
                String::new()
            } else {
                format!(" variant={}", variant.tsx())
            };
            format!(
                "button {label:?} action={}{variant}{}",
                number(&numbers.actions, action),
                looked(*look)
            )
        }
        NodeKind::Slot { slot, style } => {
            format!("slot {}{}", number(&numbers.slots, slot), styled(*style))
        }
        NodeKind::Progress { slot } => format!("progress slot {}", number(&numbers.slots, slot)),
        NodeKind::Icon { name } => format!("icon {name}"),
        NodeKind::Group(group) => format!("group {}", number(&numbers.groups, &group.name)),
        NodeKind::TextInput {
            placeholder,
            max_len,
            mode,
        } => {
            let field = node.id.as_ref().map_or(0, |id| number(&numbers.fields, id));
            let max_len = max_len
                .as_ref()
                .and_then(|l| module.resolve(l))
                .unwrap_or(crate::MAX_INPUT_LEN);
            let mut line = format!(
                "input field={field} max_len={max_len} mode={}",
                mode.as_str()
            );
            if let Some(p) = placeholder.as_ref().filter(|p| !p.is_empty()) {
                line.push_str(&format!(" placeholder={p:?}"));
            }
            line
        }
        NodeKind::Checkbox { label } => {
            let field = node.id.as_ref().map_or(0, |id| number(&numbers.fields, id));
            format!("checkbox field={field} {label:?}")
        }
        NodeKind::RadioGroup { options } => {
            let field = node.id.as_ref().map_or(0, |id| number(&numbers.fields, id));
            format!("radio_group field={field} {options:?}")
        }
        NodeKind::Select {
            options,
            placeholder,
            rows,
        } => {
            let field = node.id.as_ref().map_or(0, |id| number(&numbers.fields, id));
            format!(
                "list_picker field={field} rows={} placeholder={:?} {options:?}",
                rows.unwrap_or(5),
                placeholder.as_deref().unwrap_or("")
            )
        }
        NodeKind::ScrollView { .. } => "scroll".to_owned(),
        NodeKind::Keyboard => "keyboard".to_owned(),
        NodeKind::FieldError { field } => {
            format!("field_error field={}", number(&numbers.fields, field))
        }
        NodeKind::Spacer { size: None } => "spacer".to_owned(),
        NodeKind::Spacer { size: Some(size) } => format!("spacer size={}", px(Some(size))),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        reason = "test code: a failed check panics, and tests index and compute on known data"
    )]

    use super::*;
    use crate::{Container, TextRole, Token};

    #[test]
    fn numbers_groups_and_slots_by_first_appearance() {
        let name = |s: &str| Name::parse(s).unwrap();
        let node = |kind| Node {
            kind,
            id: None,
            loc: None,
        };
        let group = |n: &str, children| {
            node(NodeKind::Group(crate::Group {
                name: name(n),
                children,
            }))
        };
        let root = node(NodeKind::Column(Container {
            children: vec![
                group(
                    "late",
                    vec![node(NodeKind::Slot {
                        slot: name("note"),
                        style: TextStyle::new(TextRole::Display),
                    })],
                ),
                group(
                    "ready",
                    vec![
                        node(NodeKind::Slot {
                            slot: name("when"),
                            style: TextStyle::BODY,
                        }),
                        node(NodeKind::Spacer { size: None }),
                    ],
                ),
                group("late", vec![node(NodeKind::Spacer { size: None })]),
            ],
            ..Container::default()
        }));
        let module = Module {
            source_name: "t.tsx".into(),
            tokens: Vec::new(),
            screens: vec![Screen {
                name: name("S"),
                root,
                loc: None,
            }],
        };
        assert_eq!(
            outline(&module, module.screens.first().unwrap()),
            "column gap=0 padding=0\n  group 1\n    slot 1 role=display weight=bold\n  group 2\n    slot 2\n    spacer\n  group 1\n    spacer\n"
        );
    }

    #[test]
    fn outlines_a_small_screen() {
        let name = |s: &str| Name::parse(s).unwrap();
        let node = |kind| Node {
            kind,
            id: None,
            loc: None,
        };
        let root = node(NodeKind::Column(Container {
            gap: Some(Length::Token(name("GAP"))),
            padding: None,
            surface: None,
            children: vec![
                node(NodeKind::Text {
                    text: "Hi \"there\"".into(),
                    style: TextStyle::BODY,
                }),
                node(NodeKind::Text {
                    text: "Small print".into(),
                    style: TextStyle::new(TextRole::Caption).bold(),
                }),
                node(NodeKind::Button {
                    label: "Next".into(),
                    action: name("next"),
                    variant: ButtonVariant::Primary,
                    look: crate::Look::default(),
                }),
                node(NodeKind::Row(Container {
                    children: vec![
                        node(NodeKind::Button {
                            label: "Back".into(),
                            action: name("back"),
                            variant: ButtonVariant::Secondary,
                            look: crate::Look::default(),
                        }),
                        node(NodeKind::Button {
                            label: "Next again".into(),
                            action: name("next"),
                            variant: ButtonVariant::Quiet,
                            look: crate::Look::default(),
                        }),
                    ],
                    surface: Some(crate::Surface::Header),
                    ..Container::default()
                })),
                node(NodeKind::TextInput {
                    placeholder: Some("Name".into()),
                    max_len: None,
                    mode: crate::InputMode::Text,
                }),
                node(NodeKind::Spacer {
                    size: Some(Length::Px(4)),
                }),
            ],
            ..Container::default()
        }));
        let module = Module {
            source_name: "t.tsx".into(),
            tokens: vec![Token {
                name: name("GAP"),
                value: 8,
                loc: None,
            }],
            screens: vec![Screen {
                name: name("T"),
                root,
                loc: None,
            }],
        };
        let expected = "\
column gap=8 padding=0
  text \"Hi \\\"there\\\"\"
  text \"Small print\" role=caption weight=bold
  button \"Next\" action=1
  row gap=0 padding=0 surface=header
    button \"Back\" action=2 variant=secondary
    button \"Next again\" action=1 variant=quiet
  input field=0 max_len=64 mode=text placeholder=\"Name\"
  spacer size=4
";
        assert_eq!(outline(&module, module.screens.first().unwrap()), expected);
    }
}

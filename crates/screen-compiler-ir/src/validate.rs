// SPDX-License-Identifier: MIT
//! Structural checks every IR module must pass before code is generated.

use std::collections::BTreeMap;

use crate::{
    Code, Diagnostic, Length, Loc, MAX_LINES, Module, Name, NodeKind, Screen, TextRole, TextStyle,
    Weight,
};

/// The deepest element nesting the IR accepts. The analyser's own default
/// limit is lower; this bound keeps every tree walk over the IR bounded.
pub const MAX_DEPTH: usize = 64;

/// Checks the invariants code generators rely on:
///
/// * design tokens, screens, element ids (per screen) and actions (per
///   screen) have distinct names once split into words, so no two of them can
///   collide in generated code;
/// * each slot of a screen has a name of its own: one prop fills one slot,
///   as NoWayOS's builder refuses a slot number used twice (a prop may show
///   or hide several groups, which share its number); and two different
///   props never give the same slot or group name once split into words
///   (`isReady` and `is_ready`), as two callbacks may not give one action;
/// * a group holds one or two children;
/// * text is set in a weight its role is drawn in, and every character of
///   static text is one its role's glyph atlases draw;
/// * every value-holding control (text input, checkbox, radio group) has an
///   id, which names its field;
/// * every token reference resolves;
/// * no screen nests elements deeper than [`MAX_DEPTH`].
///
/// Returns one error per problem, in no particular order.
#[must_use]
pub fn validate(module: &Module) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let mut tokens = Seen::default();
    for token in &module.tokens {
        if let Some(first) = tokens.insert(&token.name, token.loc) {
            diagnostics.push(Diagnostic::error(
                Code::DuplicateName,
                token.loc,
                format!(
                    "design token `{}` has the same name as `{}`{}",
                    token.name,
                    first.name,
                    at(first.loc)
                ),
            ));
        }
    }

    let mut screens = Seen::default();
    for screen in &module.screens {
        if let Some(first) = screens.insert(&screen.name, screen.loc) {
            diagnostics.push(Diagnostic::error(
                Code::DuplicateName,
                screen.loc,
                format!(
                    "screen `{}` has the same name as `{}`{}",
                    screen.name,
                    first.name,
                    at(first.loc)
                ),
            ));
        }
        validate_screen(module, screen, &mut diagnostics);
    }

    diagnostics
}

fn validate_screen(module: &Module, screen: &Screen, diagnostics: &mut Vec<Diagnostic>) {
    let mut ids = Seen::default();
    let mut actions = Seen::default();
    let mut slots = Seen::default();
    let mut groups = Seen::default();
    let mut too_deep = false;
    // Each node with its depth, and whether a scroll view holds it.
    let mut scroll_views = 0_usize;
    // The first keyboard, and whether the screen has a text input for it.
    let mut keyboard: Option<Option<Loc>> = None;
    let mut text_input = false;
    // The controls' ids, and each field's error lines, in document order.
    let mut field_ids: Vec<&Name> = Vec::new();
    let mut field_errors: Vec<(&Name, Option<Loc>)> = Vec::new();
    let mut stack = vec![(&screen.root, 1_usize, false)];
    while let Some((node, depth, scrolled)) = stack.pop() {
        let is_scroll = matches!(node.kind, NodeKind::ScrollView { .. });
        if is_scroll {
            scroll_views = scroll_views.saturating_add(1);
            if scrolled {
                diagnostics.push(
                    Diagnostic::error(
                        Code::InvalidChildren,
                        node.loc,
                        "a <ScrollView> cannot hold another",
                    )
                    .with_help("a screen scrolls in one place: keep the outer one"),
                );
            } else if scroll_views > 1 {
                diagnostics.push(
                    Diagnostic::error(
                        Code::Limit,
                        node.loc,
                        format!("screen `{}` has more than one <ScrollView>", screen.name),
                    )
                    .with_help(
                        "a screen scrolls in one place: put what scrolls in one <ScrollView>",
                    ),
                );
            }
        }
        match node.kind {
            NodeKind::TextInput { .. } => text_input = true,
            NodeKind::Keyboard if keyboard.is_some() => diagnostics.push(
                Diagnostic::error(
                    Code::Limit,
                    node.loc,
                    format!("screen `{}` has more than one <Keyboard>", screen.name),
                )
                .with_help("one keyboard types into whichever text input has focus"),
            ),
            NodeKind::Keyboard => keyboard = Some(node.loc),
            NodeKind::FieldError { ref field } => field_errors.push((field, node.loc)),
            _ => {}
        }
        if node.kind.is_field()
            && let Some(id) = &node.id
        {
            field_ids.push(id);
        }
        if depth > MAX_DEPTH {
            if !too_deep {
                too_deep = true;
                diagnostics.push(Diagnostic::error(
                    Code::Limit,
                    node.loc,
                    format!(
                        "screen `{}` nests elements more than {MAX_DEPTH} levels deep",
                        screen.name
                    ),
                ));
            }
            continue;
        }
        if let Some(id) = &node.id
            && let Some(first) = ids.insert(id, node.loc)
        {
            diagnostics.push(Diagnostic::error(
                Code::DuplicateName,
                node.loc,
                format!(
                    "id `{id}` has the same name as id `{}`{} in screen `{}`",
                    first.name,
                    at(first.loc),
                    screen.name
                ),
            ));
        }
        let lengths: Vec<&Length> = match &node.kind {
            NodeKind::Column(c) | NodeKind::Row(c) => {
                c.gap.iter().chain(c.padding.iter()).collect()
            }
            NodeKind::Spacer { size } => size.iter().collect(),
            NodeKind::TextInput { max_len, .. } => max_len.iter().collect(),
            _ => Vec::new(),
        };
        if node.kind.is_field() && node.id.is_none() {
            diagnostics.push(Diagnostic::error(
                Code::Internal,
                node.loc,
                format!(
                    "a <{}> without an id has no field; please report this input",
                    node.component()
                ),
            ));
        }
        // A theme's spacing token measures space; it is no number of bytes.
        if let NodeKind::TextInput {
            max_len: Some(Length::Space(_)),
            ..
        } = &node.kind
        {
            diagnostics.push(Diagnostic::error(
                Code::InvalidValue,
                node.loc,
                "a spacing token is not a number of bytes; please report this input".to_owned(),
            ));
        }
        for length in lengths {
            if let Length::Token(name) = length
                && module.token(name).is_none()
            {
                diagnostics.push(Diagnostic::error(
                    Code::UnknownName,
                    node.loc,
                    format!("unknown design token `{name}`"),
                ));
            }
        }
        if let NodeKind::Button { action, .. } = &node.kind
            && let Some(first) = actions.insert(action, node.loc)
            && first.name.source() != action.source()
        {
            diagnostics.push(Diagnostic::error(
                Code::DuplicateName,
                node.loc,
                format!(
                    "callback props `{}` and `{}` would get the same action name in screen `{}`",
                    first.name.source(),
                    action.source(),
                    screen.name
                ),
            ));
        }
        if let NodeKind::Icon { name } = &node.kind
            && !crate::ICON_NAMES.contains(&name.as_str())
        {
            diagnostics.push(Diagnostic::error(
                Code::InvalidValue,
                node.loc,
                format!("there is no icon named `{name}`"),
            ));
        }
        if let NodeKind::Text { style, .. } | NodeKind::Slot { style, .. } = &node.kind {
            check_style(*style, node.loc, diagnostics);
        }
        let (texts, role): (Vec<&str>, TextRole) = match &node.kind {
            NodeKind::Text { text, style } => (vec![text], style.role),
            NodeKind::Button { label, .. } | NodeKind::Checkbox { label } => {
                (vec![label], TextRole::Body)
            }
            NodeKind::TextInput { placeholder, .. } => (
                placeholder.iter().map(String::as_str).collect(),
                TextRole::Body,
            ),
            NodeKind::RadioGroup { options } => {
                (options.iter().map(String::as_str).collect(), TextRole::Body)
            }
            NodeKind::Select {
                options,
                placeholder,
                ..
            } => (
                options
                    .iter()
                    .chain(placeholder.iter())
                    .map(String::as_str)
                    .collect(),
                TextRole::Body,
            ),
            _ => (Vec::new(), TextRole::Body),
        };
        if let Some(c) = texts
            .iter()
            .flat_map(|text| text.chars())
            .find(|&c| !role.draws(c))
        {
            diagnostics.push(Diagnostic::error(
                Code::Unsupported,
                node.loc,
                format!(
                    "a <{}> in the {role} role holds {c:?} (U+{:04X}), which its glyph atlases do not draw",
                    node.component(),
                    u32::from(c)
                ),
            ));
        }
        if let NodeKind::Slot { slot, .. } | NodeKind::Progress { slot } = &node.kind
            && let Some(first) = slots.insert(slot, node.loc)
        {
            let diagnostic = if first.name.source() == slot.source() {
                Diagnostic::error(
                    Code::DuplicateName,
                    node.loc,
                    format!(
                        "the prop `{slot}` is already the text of another <Text>{} in screen `{}`; a prop fills one",
                        at(first.loc),
                        screen.name
                    ),
                )
                .with_help("pass the text a second time, as a second prop")
            } else {
                Diagnostic::error(
                    Code::DuplicateName,
                    node.loc,
                    format!(
                        "props `{}`{} and `{slot}` would get the same slot name in screen `{}`",
                        first.name.source(),
                        at(first.loc),
                        screen.name
                    ),
                )
                .with_help("rename one of the props")
            };
            diagnostics.push(diagnostic);
        }
        // One prop may show several groups, which share its number; two
        // props whose names give the same group name may not.
        if let NodeKind::Group(group) = &node.kind
            && let Some(first) = groups.insert(&group.name, node.loc)
            && first.name.source() != group.name.source()
        {
            diagnostics.push(
                Diagnostic::error(
                    Code::DuplicateName,
                    node.loc,
                    format!(
                        "props `{}`{} and `{}` would get the same group name in screen `{}`",
                        first.name.source(),
                        at(first.loc),
                        group.name.source(),
                        screen.name
                    ),
                )
                .with_help("rename one of the props"),
            );
        }
        if let NodeKind::Group(group) = &node.kind
            && !(1..=2).contains(&group.children.len())
        {
            diagnostics.push(Diagnostic::error(
                Code::Internal,
                node.loc,
                format!(
                    "group `{}` holds {} elements, not one or two; please report this input",
                    group.name,
                    group.children.len()
                ),
            ));
        }
        let next = depth.saturating_add(1);
        let inside = scrolled || is_scroll;
        stack.extend(node.children().iter().rev().map(|c| (c, next, inside)));
    }
    for (index, &(field, loc)) in field_errors.iter().enumerate() {
        if !field_ids.iter().any(|id| id.same_words(field)) {
            diagnostics.push(
                Diagnostic::error(
                    Code::UnknownName,
                    loc,
                    format!(
                        "<FieldError for=\"{field}\"> names no control on screen `{}`",
                        screen.name
                    ),
                )
                .with_help("`for` is the id of a <TextInput>, <Checkbox>, <RadioGroup> or <Select> on the same screen"),
            );
        } else if field_errors
            .get(..index)
            .is_some_and(|before| before.iter().any(|(earlier, _)| earlier.same_words(field)))
        {
            diagnostics.push(
                Diagnostic::error(
                    Code::DuplicateName,
                    loc,
                    format!("field `{field}` already has a <FieldError>"),
                )
                .with_help("a field's errors show on one line: keep the first"),
            );
        }
    }
    if let Some(loc) = keyboard
        && !text_input
    {
        diagnostics.push(
            Diagnostic::error(
                Code::InvalidChildren,
                loc,
                format!(
                    "a <Keyboard> types into a text input, and screen `{}` has none",
                    screen.name
                ),
            )
            .with_help("add a <TextInput> for it to type into, or leave the keyboard out"),
        );
    }
}

/// Reports a style in a weight its role is not drawn in - display, title
/// or heading in Regular - and one wrapping onto fewer than 1 or more than
/// [`MAX_LINES`] lines.
fn check_style(style: TextStyle, loc: Option<Loc>, diagnostics: &mut Vec<Diagnostic>) {
    if style.role.bold_only() && style.weight == Weight::Regular {
        diagnostics.push(Diagnostic::error(
            Code::InvalidValue,
            loc,
            format!("the {} role is drawn only in Bold", style.role),
        ));
    }
    if !(1..=MAX_LINES).contains(&style.lines) {
        diagnostics.push(Diagnostic::error(
            Code::InvalidValue,
            loc,
            format!(
                "a text wraps onto 1 to {MAX_LINES} lines, not {}",
                style.lines
            ),
        ));
    }
}

/// The first name seen for each word list.
#[derive(Default)]
struct Seen<'a> {
    by_words: BTreeMap<&'a [String], First<'a>>,
}

#[derive(Clone, Copy)]
struct First<'a> {
    name: &'a Name,
    loc: Option<Loc>,
}

impl<'a> Seen<'a> {
    /// Records `name`; returns the earlier name with the same words, if any.
    fn insert(&mut self, name: &'a Name, loc: Option<Loc>) -> Option<First<'a>> {
        match self.by_words.get(name.words()) {
            Some(first) => Some(*first),
            None => {
                self.by_words.insert(name.words(), First { name, loc });
                None
            }
        }
    }
}

fn at(loc: Option<Loc>) -> String {
    loc.map(|l| format!(" (at {l})")).unwrap_or_default()
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
    use crate::{Container, Node, Token};

    fn name(s: &str) -> Name {
        Name::parse(s).unwrap()
    }

    fn leaf(kind: NodeKind, id: Option<&str>) -> Node {
        Node {
            kind,
            id: id.map(name),
            loc: None,
        }
    }

    fn column(children: Vec<Node>) -> Node {
        leaf(
            NodeKind::Column(Container {
                children,
                ..Container::default()
            }),
            None,
        )
    }

    fn module(tokens: Vec<Token>, root: Node) -> Module {
        Module {
            source_name: "t.tsx".into(),
            tokens,
            screens: vec![Screen {
                name: name("Test"),
                root,
                loc: None,
            }],
        }
    }

    fn text(text: &str) -> NodeKind {
        NodeKind::Text {
            text: text.into(),
            style: TextStyle::BODY,
        }
    }

    fn button(label: &str, action: &str) -> Node {
        leaf(
            NodeKind::Button {
                label: label.into(),
                action: name(action),
                variant: crate::ButtonVariant::Primary,
                look: crate::Look::default(),
            },
            None,
        )
    }

    #[test]
    fn text_its_role_cannot_draw_or_a_weight_it_is_not_drawn_in_is_reported() {
        let styled = |text: &str, style: TextStyle| {
            leaf(
                NodeKind::Text {
                    text: text.into(),
                    style,
                },
                None,
            )
        };
        let display = TextStyle::new(TextRole::Display);
        let codes = |nodes: Vec<Node>| -> Vec<Code> {
            validate(&module(Vec::new(), column(nodes)))
                .iter()
                .map(|d| d.code)
                .collect()
        };
        assert!(
            codes(vec![
                styled("A047 \u{a3}27.40", display),
                styled("Caf\u{e9}", TextStyle::new(TextRole::Caption).bold()),
            ])
            .is_empty()
        );
        assert_eq!(codes(vec![styled("A047a", display)]), [Code::Unsupported]);
        assert_eq!(
            codes(vec![styled("5 \u{20ac}", TextStyle::BODY)]),
            [Code::Unsupported]
        );
        assert_eq!(codes(vec![button("\u{2014}", "go")]), [Code::Unsupported]);
        let regular_title = TextStyle {
            weight: Weight::Regular,
            ..TextStyle::new(TextRole::Title)
        };
        assert_eq!(
            codes(vec![styled("Hi", regular_title)]),
            [Code::InvalidValue]
        );
        let slot = leaf(
            NodeKind::Slot {
                slot: name("ticket"),
                style: regular_title,
            },
            None,
        );
        assert_eq!(codes(vec![slot]), [Code::InvalidValue]);
    }

    #[test]
    fn a_valid_module_passes() {
        let token = Token {
            name: name("SPACE_MD"),
            value: 16,
            loc: None,
        };
        let root = leaf(
            NodeKind::Column(Container {
                gap: Some(Length::Token(name("SPACE_MD"))),
                padding: Some(Length::Px(4)),
                children: vec![button("Go", "go"), button("Again", "go")],
                surface: None,
                ..Container::default()
            }),
            Some("main"),
        );
        assert!(validate(&module(vec![token], root)).is_empty());
    }

    #[test]
    fn clashing_ids_and_tokens_are_reported() {
        let tokens = vec![
            Token {
                name: name("spaceMd"),
                value: 1,
                loc: Some(Loc::new(1, 1)),
            },
            Token {
                name: name("SPACE_MD"),
                value: 2,
                loc: Some(Loc::new(2, 1)),
            },
        ];
        let root = column(vec![
            leaf(text("a"), Some("full-name")),
            leaf(text("b"), Some("fullName")),
        ]);
        let found = validate(&module(tokens, root));
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(found.iter().all(|d| d.code == Code::DuplicateName));
        assert!(found.iter().any(|d| d.message.contains("(at 1:1)")));
    }

    #[test]
    fn different_callbacks_with_the_same_action_name_clash() {
        let root = column(vec![button("A", "done"), button("B", "Done")]);
        let found = validate(&module(Vec::new(), root));
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().unwrap().code, Code::DuplicateName);
    }

    #[test]
    fn unresolved_tokens_are_reported() {
        let root = leaf(
            NodeKind::Spacer {
                size: Some(Length::Token(name("MISSING"))),
            },
            None,
        );
        let found = validate(&module(Vec::new(), root));
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().unwrap().code, Code::UnknownName);
    }

    #[test]
    fn a_prop_fills_one_slot_but_may_show_several_groups() {
        let slot = |n: &str| {
            leaf(
                NodeKind::Slot {
                    slot: name(n),
                    style: TextStyle::BODY,
                },
                None,
            )
        };
        let group = |n: &str, children: Vec<Node>| {
            leaf(
                NodeKind::Group(crate::Group {
                    name: name(n),
                    children,
                }),
                None,
            )
        };
        let shared = column(vec![
            group("ready", vec![slot("note")]),
            group("ready", vec![slot("when"), slot("where")]),
        ]);
        assert!(validate(&module(Vec::new(), shared)).is_empty());
        let twice = column(vec![slot("note"), slot("note")]);
        let found = validate(&module(Vec::new(), twice));
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().unwrap().code, Code::DuplicateName);
        // Different props, one name: refused for slots and for groups.
        let clash = column(vec![
            slot("isReady"),
            slot("is_ready"),
            group("late", vec![slot("a")]),
            group("Late", vec![slot("b")]),
        ]);
        let found = validate(&module(Vec::new(), clash));
        assert_eq!(found.len(), 2, "{found:?}");
        assert!(
            found
                .iter()
                .all(|d| d.code == Code::DuplicateName && d.message.contains("would get the same"))
        );
        for children in [Vec::new(), vec![slot("a"), slot("b"), slot("c")]] {
            let found = validate(&module(Vec::new(), group("odd", children)));
            assert_eq!(found.len(), 1);
            assert_eq!(found.first().unwrap().code, Code::Internal);
        }
    }

    #[test]
    fn nesting_beyond_the_limit_is_reported_once() {
        let mut node = leaf(text("x"), None);
        for _ in 0..MAX_DEPTH {
            node = column(vec![node]);
        }
        let found = validate(&module(Vec::new(), node));
        assert_eq!(found.len(), 1);
        assert_eq!(found.first().unwrap().code, Code::Limit);
    }
}

// SPDX-License-Identifier: MIT
//! The Screen Compiler UI IR: a constrained, declarative description of screens.
//!
//! The IR is the contract between the TSX front end (`screen-compiler-tsx` and
//! `screen-compiler-analyse`) and the code generators (`screen-compiler-codegen-rust`). It
//! holds only what the subset supports: screens built from `Column`, `Row`,
//! `Text`, `Button`, `TextInput` and `Spacer`, string literals, a few simple
//! props, and design tokens as named integer constants; and, from subset v1,
//! slots for text the application sets and groups it shows or hides, each
//! named after the prop it comes from, checkboxes, radio groups, text
//! inputs with a length limit and an input mode, and text in a role of the
//! type scale and a weight ([`TextStyle`]). Anything the IR cannot
//! express exactly is reported by the analyser instead of being
//! approximated.
//!

#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod chars;
mod diag;
mod name;
mod outline;
mod style;
mod validate;

pub use chars::{SpecialChar, escape_special, special_char};
pub use diag::{Code, Diagnostic, Loc, Severity, has_errors, sort_diagnostics};
pub use name::{Name, NameError};
pub use outline::outline;
pub use style::{
    AlignItems, ButtonVariant, Colour, Fit, ICON_NAMES, Justify, Look, MAX_BORDER_WIDTH, MAX_LINES,
    MAX_RADIUS, ScrollAxes, Space, Surface, TextAlign, TextRole, TextStyle, Tone, Weight,
    is_display, is_printable,
};
pub use validate::{MAX_DEPTH, validate};

/// The most bytes a long text may hold (contract v16): NoWayOS's
/// `long_text::MAX_LONG_TEXT`.
pub const MAX_LONG_TEXT: u16 = u16::MAX;

/// The most bytes a text area may hold (contract v19): NoWayOS's
/// `area::MAX_AREA`.
pub const MAX_AREA_LEN: u16 = u16::MAX;

/// How many lines a `<TextArea>` may show (contract v19); 2 when not
/// given, as HTML's `<textarea>`.
pub const AREA_ROWS: core::ops::RangeInclusive<u8> = 1..=32;

/// One converted source file: its design tokens and screens, in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    /// The source file's name, used in generated comments. Code generators
    /// show only its last path component, restricted to characters that are
    /// safe in comments and documentation.
    pub source_name: String,
    /// The files the screens import components from, each as a path from
    /// the source file's folder, in the order first read: a location in
    /// file `n` (from 1) is in `imported[n - 1]`.
    pub imported: Vec<String>,
    /// Design tokens: named integer constants.
    pub tokens: Vec<Token>,
    /// Screens, one per screen component.
    pub screens: Vec<Screen>,
}

impl Module {
    /// The name of file `file`: the source file's for 0 (and for a number
    /// it does not have), else the imported file's.
    #[must_use]
    pub fn file_name(&self, file: u16) -> &str {
        usize::from(file)
            .checked_sub(1)
            .and_then(|index| self.imported.get(index))
            .unwrap_or(&self.source_name)
    }

    /// The token whose name has the same words as `name`.
    #[must_use]
    pub fn token(&self, name: &Name) -> Option<&Token> {
        self.tokens.iter().find(|t| t.name.same_words(name))
    }

    /// The pixel value of a length, resolving design tokens.
    #[must_use]
    pub fn resolve(&self, length: &Length) -> Option<u16> {
        match length {
            Length::Px(px) => Some(*px),
            Length::Token(name) => self.token(name).map(|t| t.value),
            // The theme measures it when NoWayOS lays the screen out:
            // no number here.
            Length::Space(_) => None,
        }
    }
}

/// A design token: a named integer constant, in logical pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    /// The constant's name as declared.
    pub name: Name,
    /// Its value.
    pub value: u16,
    /// Where it is declared.
    pub loc: Option<Loc>,
}

/// A screen: one component whose root is `<Screen>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screen {
    /// The component's name.
    pub name: Name,
    /// The single element inside `<Screen>`.
    pub root: Node,
    /// Where the component is declared.
    pub loc: Option<Loc>,
}

impl Screen {
    /// The actions this screen emits, in order of first appearance
    /// (depth-first, document order), one per distinct name. Generated code
    /// numbers actions from 1 in this order.
    #[must_use]
    pub fn actions(&self) -> Vec<&Name> {
        let mut actions: Vec<&Name> = Vec::new();
        for node in self.nodes() {
            if let NodeKind::Button { action, .. } = &node.kind
                && !actions.iter().any(|a| a.same_words(action))
            {
                actions.push(action);
            }
        }
        actions
    }

    /// The switches this screen's cases are on, in document order, one per
    /// distinct prop, each with its cases in the order they are first
    /// written. Generated code numbers switches from 1, and each switch's
    /// cases from 0, in these orders.
    #[must_use]
    pub fn switches(&self) -> Vec<(&Name, Vec<&Name>)> {
        let mut switches: Vec<(&Name, Vec<&Name>)> = Vec::new();
        for node in self.nodes() {
            if let NodeKind::Case(case) = &node.kind {
                match switches
                    .iter_mut()
                    .find(|(name, _)| name.same_words(&case.switch))
                {
                    Some((_, cases)) => {
                        if !cases.iter().any(|when| when.same_words(&case.when)) {
                            cases.push(&case.when);
                        }
                    }
                    None => switches.push((&case.switch, vec![&case.when])),
                }
            }
        }
        switches
    }

    /// The groups this screen shows or hides, in document order (depth-first,
    /// pre-order), one per distinct name. Generated code numbers groups from
    /// 1 in this order.
    #[must_use]
    pub fn groups(&self) -> Vec<&Name> {
        self.distinct(|kind| match kind {
            NodeKind::Group(group) => Some(&group.name),
            _ => None,
        })
    }

    /// The value-holding controls' ids - text inputs, checkboxes and radio
    /// groups - in document order. Generated code numbers fields from 1 in
    /// this order. (Every such control has an id, and ids are distinct.)
    #[must_use]
    pub fn fields(&self) -> Vec<&Name> {
        self.nodes()
            .into_iter()
            .filter(|node| node.kind.is_field())
            .filter_map(|node| node.id.as_ref())
            .collect()
    }

    /// The virtual lists on this screen, in document order, one per prop
    /// (contract v18). Generated code numbers lists from 1 in this order.
    #[must_use]
    pub fn lists(&self) -> Vec<&Name> {
        self.distinct(|kind| match kind {
            NodeKind::VirtualList { list, .. } => Some(list),
            _ => None,
        })
    }

    /// The slots of application text on this screen, in document order, one
    /// per distinct name. Generated code numbers slots from 1 in this order.
    #[must_use]
    pub fn slots(&self) -> Vec<&Name> {
        self.distinct(|kind| match kind {
            NodeKind::Slot { slot, .. } | NodeKind::Progress { slot } => Some(slot),
            _ => None,
        })
    }

    /// The names `pick` finds, in document order, each once.
    fn distinct<'s>(&'s self, pick: impl Fn(&'s NodeKind) -> Option<&'s Name>) -> Vec<&'s Name> {
        let mut names: Vec<&Name> = Vec::new();
        for node in self.nodes() {
            if let Some(name) = pick(&node.kind)
                && !names.iter().any(|n| n.same_words(name))
            {
                names.push(name);
            }
        }
        names
    }

    /// Every node, depth-first in document order (pre-order), stopping below
    /// [`MAX_DEPTH`] so a malformed IR cannot make the walk unbounded.
    #[must_use]
    pub fn nodes(&self) -> Vec<&Node> {
        let mut out = Vec::new();
        let mut stack: Vec<(&Node, usize)> = vec![(&self.root, 1)];
        while let Some((node, depth)) = stack.pop() {
            out.push(node);
            if depth < MAX_DEPTH {
                let next = depth.saturating_add(1);
                stack.extend(node.children().iter().rev().map(|c| (c, next)));
            }
        }
        out
    }
}

/// One element of a screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// What kind of element, with its own props and children.
    pub kind: NodeKind,
    /// The element's `id`, which generated code exposes as a handle.
    pub id: Option<Name>,
    /// Where the element starts.
    pub loc: Option<Loc>,
}

impl Node {
    /// The node's children (empty for leaf elements).
    #[must_use]
    pub fn children(&self) -> &[Node] {
        match &self.kind {
            NodeKind::Column(c) | NodeKind::Row(c) => &c.children,
            NodeKind::Group(group) => &group.children,
            NodeKind::Case(case) => &case.children,
            NodeKind::ScrollView { children } => children,
            NodeKind::VirtualList { rows, .. } => rows,
            _ => &[],
        }
    }

    /// The component name, as written in TSX; for a group, the expression
    /// it comes from, in ASCII.
    #[must_use]
    pub fn component(&self) -> &'static str {
        match &self.kind {
            NodeKind::Column(c) => match c.surface {
                Some(Surface::Card) => "Card",
                Some(Surface::Notice(_)) => "Notice",
                Some(Surface::Header) | None => "Column",
            },
            NodeKind::Row(c) => match c.surface {
                Some(Surface::Header) => "HeaderBar",
                _ => "Row",
            },
            NodeKind::Text { .. } | NodeKind::Slot { .. } => "Text",
            NodeKind::Button { .. } => "Button",
            NodeKind::TextInput { .. } => "TextInput",
            NodeKind::TextArea { .. } => "TextArea",
            NodeKind::Checkbox { .. } => "Checkbox",
            NodeKind::RadioGroup { .. } => "RadioGroup",
            NodeKind::Select { .. } => "Select",
            NodeKind::Spacer { .. } => "Spacer",
            NodeKind::ScrollView { .. } => "ScrollView",
            NodeKind::Keyboard => "Keyboard",
            NodeKind::FieldError { .. } => "FieldError",
            NodeKind::Progress { .. } => "ProgressBar",
            NodeKind::Icon { .. } => "Icon",
            NodeKind::Image { .. } => "Image",
            NodeKind::VirtualList { .. } => "VirtualList",
            NodeKind::Group(group) if group.children.len() > 1 => "cond ? ... : ...",
            NodeKind::Group(group) if group.unless => "!cond && ...",
            NodeKind::Group(_) => "cond && ...",
            NodeKind::Case(case) if case.children.len() > 1 => "prop === case ? ... : ...",
            NodeKind::Case(_) => "prop === case && ...",
        }
    }
}

/// The literal text around a slot's (contract v14): `before` and `after`,
/// together at most [`MAX_TEMPLATE_BYTES`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    /// The text before the prop's.
    pub before: String,
    /// The text after it.
    pub after: String,
}

/// The most bytes a slot's literal text takes, before and after together:
/// NoWayOS's `TEMPLATE_CAPACITY`.
pub const MAX_TEMPLATE_BYTES: usize = 64;

/// The element kinds of the subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// Children stacked vertically.
    Column(Container),
    /// Children side by side.
    Row(Container),
    /// Static text.
    Text {
        /// The text exactly as React would render it.
        text: String,
        /// Its role and weight: body text, Regular, unless the
        /// TSX names a `role` or a `weight`.
        style: TextStyle,
    },
    /// A button that emits an action when pressed.
    Button {
        /// The button's label.
        label: String,
        /// The action, named after the callback prop that handles it.
        action: Name,
        /// How it is drawn: primary unless the TSX names another
        /// `variant`.
        variant: ButtonVariant,
        /// Its style's fill and label colour.
        look: Look,
    },
    /// A single-line text field.
    TextInput {
        /// Hint text shown while the field is empty.
        placeholder: Option<String>,
        /// The most bytes it takes, 1 to [`MAX_INPUT_LEN`] (subset v1:
        /// `maxLength`); the runtime's most when `None`.
        max_len: Option<Length>,
        /// Which characters it takes (subset v1: `inputMode`).
        mode: InputMode,
    },
    /// Several lines a person types (contract v19): `<TextArea rows={4}
    /// maxLength={1000} />`, wrapped at its width and scrolled to keep the
    /// caret in view, its bytes in the text store of the tree the screen is
    /// built into.
    TextArea {
        /// Hint text shown while it is empty.
        placeholder: Option<String>,
        /// How many lines show, within [`AREA_ROWS`]; 2 when `None`.
        rows: Option<u8>,
        /// The most bytes it takes, 1 to [`MAX_AREA_LEN`]: required, as
        /// they come out of the tree's text store.
        max_len: Length,
    },
    /// A box the person ticks or unticks, with its label (subset v1).
    Checkbox {
        /// The label beside the box.
        label: String,
    },
    /// One choice of a few options, one per row (subset v1).
    RadioGroup {
        /// The options, 1 to [`MAX_RADIO_OPTIONS`].
        options: Vec<String>,
    },
    /// One choice of up to [`MAX_SELECT_OPTIONS`] options in a list that
    /// scrolls inside itself: `<Select>`, NoWayOS's list picker.
    Select {
        /// The options, 1 to [`MAX_SELECT_OPTIONS`].
        options: Vec<String>,
        /// Shown until an option is chosen.
        placeholder: Option<String>,
        /// How many rows of options show at once, within [`SELECT_ROWS`];
        /// NoWayOS's default (5) when `None`.
        rows: Option<u8>,
    },
    /// The on-screen keyboard: `<Keyboard />`, its keys typing
    /// into the focused text input in a layout for its mode. At most one in
    /// a screen, and only in one with a text input ([`validate`]).
    Keyboard,
    /// Where the errors the application sets for a field show:
    /// `<FieldError for="phone" />`. Its field is a control's `id` on the
    /// screen, and a field has at most one ([`validate`]).
    FieldError {
        /// The `id` of the control whose errors it shows.
        field: Name,
    },
    /// Empty space: a fixed size, or flexible when `size` is `None`.
    Spacer {
        /// Fixed size along the parent's axis.
        size: Option<Length>,
    },
    /// One child - a column - shown through a view that scrolls:
    /// `<ScrollView><Column>…</Column></ScrollView>`. At most one in a
    /// screen, never inside another ([`validate`]).
    ScrollView {
        /// Its one child.
        children: Vec<Node>,
    },
    /// One line of text the application sets at run time (subset v1):
    /// `<Text>{props.name}</Text>`, or between literal text compiled in
    /// (contract v14): `<Text>Hello, {props.name}!</Text>`. Never the
    /// prop's text compiled in from the source.
    Slot {
        /// The prop the text comes from, which names the slot.
        slot: Name,
        /// Its role and weight, as for [`NodeKind::Text`].
        style: TextStyle,
        /// The literal text around the prop's, if any.
        template: Option<Template>,
        /// For paragraphs of any length (contract v16):
        /// `<Text maxLength={10240}>{props.article}</Text>`, the bytes it
        /// may hold, 1 to [`MAX_LONG_TEXT`]; wrapped onto as many lines as
        /// it needs. Never with a template.
        capacity: Option<Length>,
    },
    /// A virtual list (contract v18): `<VirtualList rowHeight={56}>`
    /// around one list from a prop, `{props.items.slice(0, 12).map(…)}` -
    /// its rows, built once, the pool through which NoWayOS shows as many
    /// items as the application counts, binding each row to an item as the
    /// list scrolls.
    VirtualList {
        /// The prop the rows come from, which names the list.
        list: Name,
        /// Each row's height.
        row_height: Length,
        /// The pool of rows: one for each place the list was sliced to.
        rows: Vec<Node>,
        /// Its fixed width, from its style.
        width: Option<Length>,
        /// Its fixed height, from its style.
        height: Option<Length>,
    },
    /// One of the icons: `<Icon name="fuel" />`, a name from
    /// [`ICON_NAMES`].
    Icon {
        /// Its Lucide name.
        name: String,
    },
    /// One of the product's pictures, converted at build time (contract
    /// v15): `<Image src="harbour" alt="…" fit="cover" />`, `src` one of
    /// the names the picture tool lists (`--pictures`).
    Image {
        /// The picture's name, as the product's `pictures.toml` gives it.
        picture: String,
        /// What it shows, in words: empty for decoration.
        alt: String,
        /// How it fills a box of another shape.
        fit: Fit,
        /// Its fixed width, from its style; its own when absent.
        width: Option<Length>,
        /// Its fixed height, from its style; its own when absent.
        height: Option<Length>,
    },
    /// How far something has gone, as a bar the application fills at run
    /// time: `<ProgressBar value={props.fuelled} />`. Its prop
    /// names a slot number of its own, shared with the slots' numbering.
    Progress {
        /// The prop the value comes from, which names the bar.
        slot: Name,
    },
    /// Content the application shows or hides (subset v1):
    /// `{props.ready && <X/>}` or `{props.ready ? <X/> : <Y/>}`; and, in
    /// contract v13, `{!props.ready && <X/>}`.
    Group(Group),
    /// Content shown while a prop is one case of a few (contract v13):
    /// `{props.step === "review" && <X/>}` or `… ? <X/> : <Y/>`.
    Case(Case),
}

impl NodeKind {
    /// Whether this kind holds a value the application reads by field
    /// number: a text input or area, a checkbox, a radio group or a list
    /// picker.
    #[must_use]
    pub const fn is_field(&self) -> bool {
        matches!(
            self,
            Self::TextInput { .. }
                | Self::TextArea { .. }
                | Self::Checkbox { .. }
                | Self::RadioGroup { .. }
                | Self::Select { .. }
        )
    }
}

/// The most bytes a text input may be limited to: NoWayOS's
/// `INPUT_CAPACITY`.
pub const MAX_INPUT_LEN: u16 = 64;

/// The most options a radio group may offer: NoWayOS's `MAX_OPTIONS`. More
/// need a list picker.
pub const MAX_RADIO_OPTIONS: usize = 8;

/// The most options a `<Select>` may offer: NoWayOS's
/// `MAX_LIST_OPTIONS`.
pub const MAX_SELECT_OPTIONS: usize = 64;

/// How many rows a `<Select>` may show at once: NoWayOS's
/// `LIST_ROWS`.
pub const SELECT_ROWS: core::ops::RangeInclusive<u8> = 3..=8;

/// The most bytes a checkbox's label or a radio option may hold.
pub const MAX_LABEL_BYTES: usize = 256;

/// Which characters a text input takes (subset v1's `inputMode`), and so
/// the on-screen keyboard's layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InputMode {
    /// Any text (`inputMode="text"`, or none).
    #[default]
    Text,
    /// Digits only (`inputMode="numeric"` or `"tel"`).
    Digits,
    /// An address such as an email address (`inputMode="email"`).
    Email,
}

impl InputMode {
    /// Every value the TSX `inputMode` prop takes.
    pub const NAMES: [&'static str; 4] = ["text", "numeric", "tel", "email"];

    /// The mode a TSX `inputMode` value names: `numeric` and `tel` are
    /// both digits only.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "text" => Some(Self::Text),
            "numeric" | "tel" => Some(Self::Digits),
            "email" => Some(Self::Email),
            _ => None,
        }
    }

    /// The mode's name in outlines and messages.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Digits => "digits",
            Self::Email => "email",
        }
    }
}

/// A group: the prop that controls it and the content it shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The prop whose truth shows the first child, which names the group.
    pub name: Name,
    /// The child shown while the prop is true and, for `cond ? <X/> : <Y/>`,
    /// the child shown while it is not. One or two.
    pub children: Vec<Node>,
    /// Whether its one child shows while the prop is false instead:
    /// `{!props.ready && <X/>}` (contract v13).
    pub unless: bool,
}

/// A case: content shown while a prop is one value of a few, its switch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    /// The prop, which names the switch.
    pub switch: Name,
    /// The value it is compared with, which names the case.
    pub when: Name,
    /// The child shown while the prop is the case and, for `… ? <X/> :
    /// <Y/>`, the child shown while it is not. One or two.
    pub children: Vec<Node>,
}

/// Layout props and children of a `Column` or `Row` - or of a `Card`,
/// `Notice` or `HeaderBar`, a column or row drawn on a surface.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Container {
    /// Space between children.
    pub gap: Option<Length>,
    /// Space around the children, on every side.
    pub padding: Option<Length>,
    /// Child elements, in order.
    pub children: Vec<Node>,
    /// The surface it is drawn on, if any.
    pub surface: Option<Surface>,
    /// Its fixed width, from its style.
    pub width: Option<Length>,
    /// Its fixed height, from its style.
    pub height: Option<Length>,
    /// Its least width, from its style.
    pub min_width: Option<Length>,
    /// Its greatest width, from its style.
    pub max_width: Option<Length>,
    /// Where its children sit across its main axis: stretched
    /// unless its style names another.
    pub align: Option<AlignItems>,
    /// Where its children sit along its main axis.
    pub justify: Option<Justify>,
    /// Its fill, border and corners.
    pub look: Look,
    /// Whether its style names `overflow`, `overflowX` or `overflowY` of
    /// `auto` or `scroll` (contract v17), and so which ways it scrolls: it
    /// is then a scroll view of its size holding it, which keeps the rest.
    pub overflow: Option<ScrollAxes>,
}

/// A length: a literal number of logical pixels, a screen's own design
/// token, or one of the theme's spacing tokens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Length {
    /// A literal number of pixels.
    Px(u16),
    /// A reference to a design token the screen declares (`const NAME = 16;`).
    Token(Name),
    /// The theme's spacing token (`space.md`), which NoWayOS measures when
    /// it lays the screen out.
    Space(Space),
}

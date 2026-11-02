// SPDX-License-Identifier: MIT
//! The component kit of the subset: the only elements a screen may use,
//! and every prop and style property each takes, with the values it
//! accepts. These tables are what the analyser checks against and what
//! `screen-compiler subset` prints ([`crate::subset`]).

use std::fmt;

use screen_compiler_ir::{
    AlignItems, ButtonVariant, ICON_NAMES, InputMode, Justify, MAX_BORDER_WIDTH, MAX_INPUT_LEN,
    MAX_LINES, MAX_RADIO_OPTIONS, MAX_RADIUS, MAX_SELECT_OPTIONS, SELECT_ROWS, TextAlign, TextRole,
    Tone, Weight,
};

/// What a component holds between its opening and closing tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Children {
    /// Nothing: the element closes itself (`<Spacer />`).
    None,
    /// Exactly one element.
    One,
    /// Elements, in order.
    Elements,
    /// Literal text, or one prop of the component, which becomes a slot
    /// the application sets.
    TextOrProp,
    /// Literal text: a button's label.
    Text,
}

impl Children {
    /// Its name in `screen-compiler subset`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::One => "one-element",
            Self::Elements => "elements",
            Self::TextOrProp => "text-or-prop",
            Self::Text => "text",
        }
    }
}

/// What a prop or a style property takes: every value it accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    /// An id: a string of ASCII letters, digits, `-` and `_`, starting
    /// with a letter.
    Id,
    /// The id of a field on the same screen.
    FieldId,
    /// A string literal.
    Text,
    /// A whole number of layout units, a design token the screen
    /// declares, or one of the theme's spacing tokens (`space.md`).
    Length,
    /// A whole number of layout units or a design token: not a spacing
    /// token.
    Size,
    /// A whole number from `least` to `most`, or, where `token`, a design
    /// token.
    Whole {
        /// The least it may be.
        least: u16,
        /// The most it may be.
        most: u16,
        /// Whether a design token may stand for it.
        token: bool,
    },
    /// One of these string literals.
    Choice(&'static [&'static str]),
    /// One of the theme's colours, by name ([`screen_compiler_ir::Colour::NAMES`]).
    Colour,
    /// A callback prop of the component, which becomes an action.
    Action,
    /// A prop of the component, whose value the application sets.
    Prop,
    /// An array of 1 to `most` string literals.
    Options {
        /// The most options it may hold.
        most: usize,
    },
    /// `style={{ … }}`, naming the component's style properties.
    Style,
}

/// A prop a component takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prop {
    /// Its name, as written in TSX.
    pub name: &'static str,
    /// What it takes.
    pub value: Value,
    /// Whether the component must be given it.
    pub required: bool,
}

impl Prop {
    const fn optional(name: &'static str, value: Value) -> Self {
        Self {
            name,
            value,
            required: false,
        }
    }

    const fn required(name: &'static str, value: Value) -> Self {
        Self {
            name,
            value,
            required: true,
        }
    }
}

/// A property a component's `style={{ … }}` may name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StyleProperty {
    /// Its name, as written in TSX (`backgroundColor`).
    pub name: &'static str,
    /// What it takes.
    pub value: Value,
}

const fn styled(name: &'static str, value: Value) -> StyleProperty {
    StyleProperty { name, value }
}

const ID: Prop = Prop::optional("id", Value::Id);
/// A field's id, which the application reads and sets it by.
const FIELD_ID: Prop = Prop::required("id", Value::Id);
const GAP: Prop = Prop::optional("gap", Value::Length);
const PADDING: Prop = Prop::optional("padding", Value::Length);
const STYLE: Prop = Prop::optional("style", Value::Style);

/// The style of a column or a row, on a surface or not.
const CONTAINER_STYLE: &[StyleProperty] = &[
    styled("gap", Value::Length),
    styled("padding", Value::Length),
    styled("width", Value::Size),
    styled("height", Value::Size),
    styled("minWidth", Value::Size),
    styled("maxWidth", Value::Size),
    styled("alignItems", Value::Choice(&AlignItems::NAMES)),
    styled("justifyContent", Value::Choice(&Justify::NAMES)),
    styled("backgroundColor", Value::Colour),
    styled("borderColor", Value::Colour),
    styled(
        "borderWidth",
        Value::Whole {
            least: 0,
            most: MAX_BORDER_WIDTH,
            token: false,
        },
    ),
    styled(
        "borderRadius",
        Value::Whole {
            least: 0,
            most: MAX_RADIUS,
            token: false,
        },
    ),
];

/// A component of the kit screens import from the kit module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum KitComponent {
    /// The root of every screen; exactly one child element.
    Screen,
    /// Children stacked vertically.
    Column,
    /// Children side by side.
    Row,
    /// Literal text.
    Text,
    /// A button that emits an action.
    Button,
    /// A single-line text field.
    TextInput,
    /// Empty space.
    Spacer,
    /// A box the person ticks (subset v1).
    Checkbox,
    /// One choice of a few options (subset v1).
    RadioGroup,
    /// Children stacked vertically on a card.
    Card,
    /// Children stacked vertically on a notice, in a tone.
    Notice,
    /// Children side by side on a header bar.
    HeaderBar,
    /// A bar the application fills, from a prop.
    ProgressBar,
    /// One of Lucide's icons, by name.
    Icon,
    /// One child shown through a view that scrolls: at most one
    /// in a screen, never nested.
    ScrollView,
    /// One choice of up to 64 options in a list that scrolls inside itself.
    Select,
    /// The on-screen keyboard: at most one in a screen, and only in
    /// one with a text input.
    Keyboard,
    /// Where the errors the application sets for a field show.
    FieldError,
}

impl KitComponent {
    /// Every component, in documentation order.
    pub const ALL: [Self; 18] = [
        Self::Screen,
        Self::Column,
        Self::Row,
        Self::Text,
        Self::Button,
        Self::TextInput,
        Self::Spacer,
        Self::Checkbox,
        Self::RadioGroup,
        Self::Card,
        Self::Notice,
        Self::HeaderBar,
        Self::ProgressBar,
        Self::Icon,
        Self::ScrollView,
        Self::Select,
        Self::Keyboard,
        Self::FieldError,
    ];

    /// Every component's name, for help lines: "Screen, Column, … and
    /// RadioGroup", with `last` (`and`, `or`) before the last.
    #[must_use]
    pub fn list(last: &str) -> String {
        let names: Vec<&str> = Self::ALL.iter().map(|c| c.name()).collect();
        match names.split_last() {
            Some((final_name, rest)) => format!("{} {last} {final_name}", rest.join(", ")),
            None => String::new(),
        }
    }

    /// The component with this exported name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }

    /// The exported name, as used in TSX.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Screen => "Screen",
            Self::Column => "Column",
            Self::Row => "Row",
            Self::Text => "Text",
            Self::Button => "Button",
            Self::TextInput => "TextInput",
            Self::Spacer => "Spacer",
            Self::Checkbox => "Checkbox",
            Self::RadioGroup => "RadioGroup",
            Self::Card => "Card",
            Self::Notice => "Notice",
            Self::HeaderBar => "HeaderBar",
            Self::ProgressBar => "ProgressBar",
            Self::Icon => "Icon",
            Self::ScrollView => "ScrollView",
            Self::Select => "Select",
            Self::Keyboard => "Keyboard",
            Self::FieldError => "FieldError",
        }
    }

    /// What it is, in a sentence: for documentation and editors.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::Screen => "The root of every screen: exactly one child element.",
            Self::Column => "Children stacked vertically.",
            Self::Row => "Children side by side.",
            Self::Text => "Text: literal, or one prop the application sets.",
            Self::Button => "A button that emits an action.",
            Self::TextInput => "A single-line text field.",
            Self::Spacer => "Empty space: flexible, or of a fixed size.",
            Self::Checkbox => "A box the person ticks.",
            Self::RadioGroup => "One choice of a few options.",
            Self::Card => "Children stacked vertically on a card.",
            Self::Notice => "Children stacked vertically on a notice, in a tone.",
            Self::HeaderBar => "Children side by side on a header bar.",
            Self::ProgressBar => "A bar the application fills, from a prop.",
            Self::Icon => "One of Lucide's icons, by name.",
            Self::ScrollView => {
                "One child shown through a view that scrolls: at most one in a screen, never nested."
            }
            Self::Select => "One choice of up to 64 options in a list that scrolls inside itself.",
            Self::Keyboard => {
                "The on-screen keyboard: at most one in a screen, and only in one with a text input."
            }
            Self::FieldError => "Where the errors the application sets for a field show.",
        }
    }

    /// Whether it holds child elements: a column or a row, on a surface or
    /// not.
    #[must_use]
    pub const fn is_container(self) -> bool {
        matches!(
            self,
            Self::Column | Self::Row | Self::Card | Self::Notice | Self::HeaderBar
        )
    }

    /// What it holds between its tags.
    #[must_use]
    pub const fn children(self) -> Children {
        match self {
            Self::Screen | Self::ScrollView => Children::One,
            Self::Column | Self::Row | Self::Card | Self::Notice | Self::HeaderBar => {
                Children::Elements
            }
            Self::Text => Children::TextOrProp,
            Self::Button => Children::Text,
            Self::TextInput
            | Self::Spacer
            | Self::Checkbox
            | Self::RadioGroup
            | Self::ProgressBar
            | Self::Icon
            | Self::Select
            | Self::Keyboard
            | Self::FieldError => Children::None,
        }
    }

    /// The props the component accepts in subset v1.
    #[must_use]
    pub const fn props(self) -> &'static [Prop] {
        match self {
            Self::Screen => &[],
            Self::Column | Self::Row | Self::Card | Self::HeaderBar => &[ID, GAP, PADDING, STYLE],
            Self::Notice => {
                const {
                    &[
                        ID,
                        GAP,
                        PADDING,
                        Prop::required("tone", Value::Choice(&Tone::NAMES)),
                        STYLE,
                    ]
                }
            }
            Self::Text => {
                const {
                    &[
                        ID,
                        Prop::optional("role", Value::Choice(&TextRole::NAMES)),
                        Prop::optional("weight", Value::Choice(&Weight::NAMES)),
                        Prop::optional(
                            "lines",
                            Value::Whole {
                                least: 1,
                                most: MAX_LINES as u16,
                                token: false,
                            },
                        ),
                        Prop::optional("align", Value::Choice(&TextAlign::NAMES)),
                        STYLE,
                    ]
                }
            }
            Self::Button => {
                const {
                    &[
                        ID,
                        Prop::required("onPress", Value::Action),
                        Prop::optional("variant", Value::Choice(&ButtonVariant::NAMES)),
                        STYLE,
                    ]
                }
            }
            Self::TextInput => {
                const {
                    &[
                        FIELD_ID,
                        Prop::optional("placeholder", Value::Text),
                        Prop::optional(
                            "maxLength",
                            Value::Whole {
                                least: 1,
                                most: MAX_INPUT_LEN,
                                token: true,
                            },
                        ),
                        Prop::optional("inputMode", Value::Choice(&InputMode::NAMES)),
                    ]
                }
            }
            Self::Spacer => const { &[ID, Prop::optional("size", Value::Length)] },
            Self::Checkbox => const { &[FIELD_ID, Prop::required("label", Value::Text)] },
            Self::RadioGroup => {
                const {
                    &[
                        FIELD_ID,
                        Prop::required(
                            "options",
                            Value::Options {
                                most: MAX_RADIO_OPTIONS,
                            },
                        ),
                    ]
                }
            }
            Self::ProgressBar => const { &[ID, Prop::required("value", Value::Prop)] },
            Self::Icon => const { &[ID, Prop::required("name", Value::Choice(&ICON_NAMES))] },
            Self::ScrollView | Self::Keyboard => &[ID],
            Self::FieldError => const { &[ID, Prop::required("for", Value::FieldId)] },
            Self::Select => {
                const {
                    &[
                        FIELD_ID,
                        Prop::optional("placeholder", Value::Text),
                        Prop::required(
                            "options",
                            Value::Options {
                                most: MAX_SELECT_OPTIONS,
                            },
                        ),
                        Prop::optional(
                            "rows",
                            Value::Whole {
                                least: *SELECT_ROWS.start() as u16,
                                most: *SELECT_ROWS.end() as u16,
                                token: false,
                            },
                        ),
                    ]
                }
            }
        }
    }

    /// The prop of this name, if the component takes it.
    #[must_use]
    pub fn prop(self, name: &str) -> Option<Prop> {
        self.props().iter().find(|p| p.name == name).copied()
    }

    /// The properties a `style={{…}}` on the component may name:
    /// none where it takes no `style`.
    #[must_use]
    pub const fn style(self) -> &'static [StyleProperty] {
        match self {
            Self::Column | Self::Row | Self::Card | Self::Notice | Self::HeaderBar => {
                CONTAINER_STYLE
            }
            Self::Text => {
                const {
                    &[
                        styled("color", Value::Colour),
                        styled("textAlign", Value::Choice(&TextAlign::STYLE_NAMES)),
                    ]
                }
            }
            Self::Button => {
                const {
                    &[
                        styled("backgroundColor", Value::Colour),
                        styled("color", Value::Colour),
                    ]
                }
            }
            _ => &[],
        }
    }

    /// Whether its style may name `property`.
    #[must_use]
    pub fn styles(self, property: &str) -> bool {
        self.style().iter().any(|p| p.name == property)
    }

    /// The names of its style's properties, for messages.
    #[must_use]
    pub fn style_names(self) -> Vec<&'static str> {
        self.style().iter().map(|p| p.name).collect()
    }

    /// Advice for a prop the component does not accept.
    #[must_use]
    pub fn unknown_prop_help(self, prop: &str) -> String {
        match (self, prop) {
            (_, "style") => {
                "a style is taken by Column, Row, Card, Notice, HeaderBar, Text and Button".to_owned()
            }
            (_, "className" | "class" | "sx") => {
                "Screen Compiler does not implement CSS classes: write style={{ … }} with the declared properties, on Column, Row, Card, Notice, HeaderBar, Text or Button"
                    .to_owned()
            }
            (Self::Button, "onClick") => "use onPress={…} with a callback prop".to_owned(),
            (_, "key") => "keys matter only in lists, which are outside subset v1".to_owned(),
            (Self::TextInput, "value" | "defaultValue" | "onChange") => {
                "controlled inputs need state, which is outside subset v1; the application reads the field through its id"
                    .to_owned()
            }
            (Self::Checkbox, "checked" | "defaultChecked" | "onChange")
            | (Self::RadioGroup, "value" | "defaultValue" | "onChange") => {
                "controlled inputs need state, which is outside subset v1; the application reads and sets the field through its id"
                    .to_owned()
            }
            (Self::TextInput, "type") => {
                "masked entry (passwords, PINs) is not in subset v1: do not remove `type` to make a screen convert; for a type that is not secret, use inputMode (text, numeric, tel or email) and maxLength"
                    .to_owned()
            }
            (Self::TextInput, "pattern" | "minLength") => {
                "use inputMode (text, numeric, tel or email) and maxLength; the application checks the rest"
                    .to_owned()
            }
            (_, "disabled" | "required" | "autoFocus") => {
                "the application decides what is required and where focus goes; subset v1 has no such prop"
                    .to_owned()
            }
            (_, "children") => "write children between the opening and closing tags".to_owned(),
            (Self::Screen, _) => "<Screen> takes no props in subset v1".to_owned(),
            _ => {
                let names: Vec<&str> = self.props().iter().map(|p| p.name).collect();
                format!("props of <{}> in subset v1: {}", self.name(), names.join(", "))
            }
        }
    }
}

impl fmt::Display for KitComponent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for c in KitComponent::ALL {
            assert_eq!(KitComponent::from_name(c.name()), Some(c));
        }
        assert_eq!(KitComponent::from_name("div"), None);
    }

    #[test]
    fn the_list_pickers_summary_gives_its_most_options() {
        let most = screen_compiler_ir::MAX_SELECT_OPTIONS.to_string();
        assert!(KitComponent::Select.summary().contains(&most));
    }

    #[test]
    fn a_component_with_a_style_takes_the_style_prop_and_no_other_does() {
        for c in KitComponent::ALL {
            assert_eq!(c.prop("style").is_some(), !c.style().is_empty(), "{c}");
        }
    }

    #[test]
    fn no_prop_or_style_property_is_listed_twice() {
        for c in KitComponent::ALL {
            let mut props: Vec<&str> = c.props().iter().map(|p| p.name).collect();
            props.sort_unstable();
            props.dedup();
            assert_eq!(props.len(), c.props().len(), "{c}");
            let mut style = c.style_names();
            style.sort_unstable();
            style.dedup();
            assert_eq!(style.len(), c.style().len(), "{c}");
        }
    }
}

// SPDX-License-Identifier: MIT
//! The component kit of the subset: the only elements a screen may use.

use std::fmt;

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

    /// Whether it holds child elements: a column or a row, on a surface or
    /// not.
    #[must_use]
    pub const fn is_container(self) -> bool {
        matches!(
            self,
            Self::Column | Self::Row | Self::Card | Self::Notice | Self::HeaderBar
        )
    }

    /// The props the component accepts in subset v1.
    #[must_use]
    pub const fn props(self) -> &'static [&'static str] {
        match self {
            Self::Screen => &[],
            Self::Column | Self::Row | Self::Card | Self::HeaderBar => {
                &["id", "gap", "padding", "style"]
            }
            Self::Notice => &["id", "gap", "padding", "tone", "style"],
            Self::Text => &["id", "role", "weight", "lines", "align", "style"],
            Self::Button => &["id", "onPress", "variant", "style"],
            Self::TextInput => &["id", "placeholder", "maxLength", "inputMode"],
            Self::Spacer => &["id", "size"],
            Self::Checkbox => &["id", "label"],
            Self::RadioGroup => &["id", "options"],
            Self::ProgressBar => &["id", "value"],
            Self::Icon => &["id", "name"],
            Self::ScrollView | Self::Keyboard => &["id"],
            Self::FieldError => &["id", "for"],
            Self::Select => &["id", "placeholder", "options", "rows"],
        }
    }

    /// The properties a `style={{…}}` on the component may name:
    /// none where it takes no `style`.
    #[must_use]
    pub const fn style_properties(self) -> &'static [&'static str] {
        match self {
            Self::Column | Self::Row | Self::Card | Self::Notice | Self::HeaderBar => &[
                "gap",
                "padding",
                "width",
                "height",
                "minWidth",
                "maxWidth",
                "alignItems",
                "justifyContent",
                "backgroundColor",
                "borderColor",
                "borderWidth",
                "borderRadius",
            ],
            Self::Text => &["color", "textAlign"],
            Self::Button => &["backgroundColor", "color"],
            _ => &[],
        }
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
            _ => format!(
                "props of <{}> in subset v1: {}",
                self.name(),
                self.props().join(", ")
            ),
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
}

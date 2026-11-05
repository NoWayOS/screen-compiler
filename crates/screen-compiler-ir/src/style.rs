// SPDX-License-Identifier: MIT
//! How text is set (subset v1): a role in the type scale and a
//! weight, and which characters each role's glyph atlases draw; and how
//! buttons and containers look: a button's variant and the surface
//! a container is drawn on.
//!
//! The device draws text from glyph atlases rasterised at build time, so a
//! character outside a role's atlas cannot be shown at all. Screen Compiler
//! refuses such text when it converts a screen, naming where it is, rather
//! than leaving the runtime to refuse it when the screen is built. The two
//! sets below are NoWayOS's (`nowayos_ui::text::is_printable` and
//! `is_display`); the contract check tests that they stay the same.

use core::fmt;

/// What a line of text is for: its size in the theme's type scale. The
/// TSX `role` prop of `<Text>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextRole {
    /// A number or a short reference to read across a room - a queue
    /// number, an amount - in digits, capitals and a few signs only
    /// ([`is_display`]). Bold.
    Display,
    /// A screen's title. Bold.
    Title,
    /// A heading: a header bar's or a section's. Bold.
    Heading,
    /// Text, fields and buttons: the default.
    #[default]
    Body,
    /// A field's or a tile's label, smaller than body.
    Label,
    /// Small print and footnotes.
    Caption,
}

impl TextRole {
    /// Every role, largest first.
    pub const ALL: [Self; 6] = [
        Self::Display,
        Self::Title,
        Self::Heading,
        Self::Body,
        Self::Label,
        Self::Caption,
    ];

    /// Every role's TSX `role` value, largest first.
    pub const NAMES: [&'static str; 6] =
        ["display", "title", "heading", "body", "label", "caption"];

    /// The role a TSX `role` value names: `display`, `title`, `heading`,
    /// `body`, `label` or `caption`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.tsx() == value)
    }

    /// Its TSX `role` value.
    #[must_use]
    pub const fn tsx(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Title => "title",
            Self::Heading => "heading",
            Self::Body => "body",
            Self::Label => "label",
            Self::Caption => "caption",
        }
    }

    /// Its name in the target API (`ui::TextRole::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Display => "Display",
            Self::Title => "Title",
            Self::Heading => "Heading",
            Self::Body => "Body",
            Self::Label => "Label",
            Self::Caption => "Caption",
        }
    }

    /// Whether the role is drawn only in Bold: display, title and heading.
    #[must_use]
    pub const fn bold_only(self) -> bool {
        matches!(self, Self::Display | Self::Title | Self::Heading)
    }

    /// Whether its atlases draw `c`: [`is_display`] for the display role,
    /// [`is_printable`] for every other.
    #[must_use]
    pub const fn draws(self, c: char) -> bool {
        match self {
            Self::Display => is_display(c),
            Self::Title | Self::Heading | Self::Body | Self::Label | Self::Caption => {
                is_printable(c)
            }
        }
    }

    /// What its atlases draw, for messages.
    #[must_use]
    pub const fn describe_chars(self) -> &'static str {
        match self {
            Self::Display => {
                "the display role draws the digits, the capitals A to Z, the space and # % + , - . / : \u{a3} only"
            }
            Self::Title | Self::Heading | Self::Body | Self::Label | Self::Caption => {
                "text is drawn from Latin-1: printable ASCII and U+00A0 to U+00FF, but not U+00AD"
            }
        }
    }
}

impl fmt::Display for TextRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tsx())
    }
}

/// A text's weight. The TSX `weight` prop of `<Text>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Weight {
    /// Regular, 400.
    #[default]
    Regular,
    /// Bold, 700.
    Bold,
}

impl Weight {
    /// Every value the TSX `weight` prop takes.
    pub const NAMES: [&'static str; 2] = ["regular", "bold"];

    /// The weight a TSX `weight` value names: `regular` or `bold`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "regular" => Some(Self::Regular),
            "bold" => Some(Self::Bold),
            _ => None,
        }
    }
}

/// Where a text's lines sit across its node: the TSX `align` prop
/// of `<Text>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TextAlign {
    /// Against the start edge: the default.
    #[default]
    Start,
    /// Centred.
    Center,
    /// Against the end edge.
    End,
}

impl TextAlign {
    /// The alignment a TSX `align` value names: `start`, `center` or `end`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "start" => Some(Self::Start),
            "center" => Some(Self::Center),
            "end" => Some(Self::End),
            _ => None,
        }
    }

    /// Every value the TSX `align` prop takes, in order.
    pub const NAMES: [&'static str; 3] = ["start", "center", "end"];

    /// Every value a style's `textAlign` takes: the prop's, and
    /// `left` and `right` for the start and the end.
    pub const STYLE_NAMES: [&'static str; 5] = ["left", "start", "center", "right", "end"];

    /// The alignment a style's `textAlign` value names.
    #[must_use]
    pub fn parse_style(value: &str) -> Option<Self> {
        match value {
            "left" => Some(Self::Start),
            "right" => Some(Self::End),
            other => Self::parse(other),
        }
    }

    /// Its TSX value: `start`, `center` or `end`.
    #[must_use]
    pub const fn tsx(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }

    /// Its name in the target API (`ui::TextAlign::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Center => "Center",
            Self::End => "End",
        }
    }
}

/// Which ways a container whose style names `overflow` scrolls (contract
/// v17): `overflowY` down, `overflowX` across, `overflow` - or both of
/// them - both ways.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ScrollAxes {
    /// Down.
    #[default]
    Vertical,
    /// Across.
    Horizontal,
    /// Both ways.
    Both,
}

impl ScrollAxes {
    /// Every value `overflow`, `overflowX` or `overflowY` takes: each
    /// scrolls.
    pub const NAMES: [&'static str; 2] = ["auto", "scroll"];

    /// Its name in the target API (`ui::ScrollAxes::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Vertical => "Vertical",
            Self::Horizontal => "Horizontal",
            Self::Both => "Both",
        }
    }

    /// Its name in the outline.
    #[must_use]
    pub const fn outline(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
            Self::Both => "both",
        }
    }
}

/// How a picture fills a box of another shape: `<Image fit="…">`, or
/// CSS's `objectFit` in its style (contract v15).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Fit {
    /// Stretched to the box: CSS's default.
    #[default]
    Fill,
    /// As large as fits inside the box, its shape kept.
    Contain,
    /// As small as covers the box, its shape kept, cut on two sides.
    Cover,
}

impl Fit {
    /// Every value it takes, in order.
    pub const NAMES: [&'static str; 3] = ["fill", "contain", "cover"];

    /// The fit a value names.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "fill" => Some(Self::Fill),
            "contain" => Some(Self::Contain),
            "cover" => Some(Self::Cover),
            _ => None,
        }
    }

    /// Its name in the target API (`ui::Fit::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Fill => "Fill",
            Self::Contain => "Contain",
            Self::Cover => "Cover",
        }
    }
}

/// One of the theme's colours, named by a style: `color`,
/// `backgroundColor` or `borderColor` take its TSX name - `"text-muted"` -
/// never a colour value, so the large-text, high-contrast theme recolours
/// the screen, and only colours whose contrast the theme's tests check are
/// drawn. NoWayOS's `nowayos_ui::Colour`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Colour {
    /// `"background"`: the screen's background.
    Background,
    /// `"text"`: ordinary text.
    Text,
    /// `"text-muted"`: text that matters less.
    TextMuted,
    /// `"placeholder"`: a text input's placeholder.
    Placeholder,
    /// `"button"`: a primary button's fill, the brand's colour.
    Button,
    /// `"button-text"`: a primary button's label.
    ButtonText,
    /// `"input"`: a text input's fill.
    Input,
    /// `"input-border"`: a text input's border.
    InputBorder,
    /// `"focus"`: the focus ring.
    Focus,
    /// `"error"`: an error.
    Error,
}

impl Colour {
    /// Every colour's TSX name, in order.
    pub const NAMES: [&'static str; 10] = [
        "background",
        "text",
        "text-muted",
        "placeholder",
        "button",
        "button-text",
        "input",
        "input-border",
        "focus",
        "error",
    ];

    const ALL: [Self; 10] = [
        Self::Background,
        Self::Text,
        Self::TextMuted,
        Self::Placeholder,
        Self::Button,
        Self::ButtonText,
        Self::Input,
        Self::InputBorder,
        Self::Focus,
        Self::Error,
    ];

    /// The colour a TSX name names.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::NAMES
            .iter()
            .position(|n| *n == name)
            .and_then(|at| Self::ALL.get(at).copied())
    }

    /// Its TSX name.
    #[must_use]
    pub fn tsx(self) -> &'static str {
        Self::ALL
            .iter()
            .position(|c| *c == self)
            .and_then(|at| Self::NAMES.get(at).copied())
            .unwrap_or_default()
    }

    /// Its name in the target API (`ui::Colour::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Background => "Background",
            Self::Text => "Text",
            Self::TextMuted => "TextMuted",
            Self::Placeholder => "Placeholder",
            Self::Button => "Button",
            Self::ButtonText => "ButtonText",
            Self::Input => "Input",
            Self::InputBorder => "InputBorder",
            Self::Focus => "Focus",
            Self::Error => "Error",
        }
    }
}

/// Where a container's children sit across its main axis: a
/// style's `alignItems`. NoWayOS's `nowayos_ui::Align`; `stretch` unless
/// named.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AlignItems {
    /// `"flex-start"` or `"start"`.
    Start,
    /// `"center"`.
    Center,
    /// `"flex-end"` or `"end"`.
    End,
    /// `"stretch"`.
    Stretch,
}

impl AlignItems {
    /// The alignment a TSX `alignItems` value names.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "flex-start" | "start" => Some(Self::Start),
            "center" => Some(Self::Center),
            "flex-end" | "end" => Some(Self::End),
            "stretch" => Some(Self::Stretch),
            _ => None,
        }
    }

    /// Every value it takes.
    pub const NAMES: [&'static str; 6] = [
        "flex-start",
        "start",
        "center",
        "flex-end",
        "end",
        "stretch",
    ];

    /// Its name in the outline.
    #[must_use]
    pub const fn tsx(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::Stretch => "stretch",
        }
    }

    /// Its name in the target API (`ui::Align::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Center => "Center",
            Self::End => "End",
            Self::Stretch => "Stretch",
        }
    }
}

/// Where a container's children sit along its main axis when they leave
/// room over: a style's `justifyContent`. NoWayOS's
/// `nowayos_ui::Justify`; from the start unless named.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Justify {
    /// `"flex-start"` or `"start"`.
    Start,
    /// `"center"`.
    Center,
    /// `"flex-end"` or `"end"`.
    End,
    /// `"space-between"`.
    SpaceBetween,
}

impl Justify {
    /// The placement a TSX `justifyContent` value names.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "flex-start" | "start" => Some(Self::Start),
            "center" => Some(Self::Center),
            "flex-end" | "end" => Some(Self::End),
            "space-between" => Some(Self::SpaceBetween),
            _ => None,
        }
    }

    /// Every value it takes.
    pub const NAMES: [&'static str; 6] = [
        "flex-start",
        "start",
        "center",
        "flex-end",
        "end",
        "space-between",
    ];

    /// Its name in the outline.
    #[must_use]
    pub const fn tsx(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
            Self::SpaceBetween => "space-between",
        }
    }

    /// Its name in the target API (`ui::Justify::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Center => "Center",
            Self::End => "End",
            Self::SpaceBetween => "SpaceBetween",
        }
    }
}

/// How a container or a button is drawn beyond its kind's own look,
/// from its style: a fill, a border and its width, its corners'
/// radius - in layout units - and a button's label colour. NoWayOS's
/// `nowayos_ui::Look`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Look {
    /// `backgroundColor`.
    pub fill: Option<Colour>,
    /// `borderColor`.
    pub border: Option<Colour>,
    /// `borderWidth`, 0 to [`MAX_BORDER_WIDTH`].
    pub border_width: Option<u16>,
    /// `borderRadius`, 0 to [`MAX_RADIUS`].
    pub radius: Option<u16>,
    /// A button's `color`: its label's.
    pub text: Option<Colour>,
    /// `boxShadow`: one of the theme's shadows (contract v22).
    pub shadow: Option<Elevation>,
    /// `background`: a gradient of the theme's colours (contract v22).
    pub gradient: Option<Gradient>,
    /// `opacity`, out of 255 (contract v22).
    pub opacity: Option<u8>,
    /// `transition`: how it comes into view when a condition shows it
    /// (contract v23).
    pub transition: Option<Transition>,
    /// `animation`: one of the runtime's, running while motion is on
    /// (contract v23).
    pub animation: Option<NamedAnimation>,
}

/// A timing function: CSS's names, `cubic-bezier(…)` in thousandths, or
/// `steps(n, jump-start | jump-end)` (contract v23).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Easing {
    /// `linear`.
    Linear,
    /// `ease`.
    Ease,
    /// `ease-in`.
    EaseIn,
    /// `ease-out`.
    EaseOut,
    /// `ease-in-out`.
    EaseInOut,
    /// `cubic-bezier(x1, y1, x2, y2)`, in thousandths.
    Bezier([i16; 4]),
    /// `steps(count, jump-start)` when `start`, else `jump-end`.
    Steps {
        /// How many.
        count: u8,
        /// Whether the first jump is at the start.
        start: bool,
    },
}

impl Easing {
    /// It in the target API (`ui::Easing::…`).
    #[must_use]
    pub fn rust(self) -> String {
        match self {
            Self::Linear => "ui::Easing::Linear".to_owned(),
            Self::Ease => "ui::Easing::Ease".to_owned(),
            Self::EaseIn => "ui::Easing::EaseIn".to_owned(),
            Self::EaseOut => "ui::Easing::EaseOut".to_owned(),
            Self::EaseInOut => "ui::Easing::EaseInOut".to_owned(),
            Self::Bezier([a, b, c, d]) => format!("ui::Easing::Bezier([{a}, {b}, {c}, {d}])"),
            Self::Steps { count, start } => {
                format!("ui::Easing::Steps {{ count: {count}, start: {start} }}")
            }
        }
    }

    /// It in an outline.
    #[must_use]
    pub fn outline(self) -> String {
        match self {
            Self::Linear => "linear".to_owned(),
            Self::Ease => "ease".to_owned(),
            Self::EaseIn => "ease-in".to_owned(),
            Self::EaseOut => "ease-out".to_owned(),
            Self::EaseInOut => "ease-in-out".to_owned(),
            Self::Bezier([a, b, c, d]) => format!("bezier-{a}-{b}-{c}-{d}"),
            Self::Steps { count, start } => {
                format!("steps-{count}-{}", if start { "start" } else { "end" })
            }
        }
    }
}

/// What a transition moves as its node comes into view (contract v23).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MotionProperty {
    /// `opacity`: from unseen.
    Opacity,
    /// `transform` or `translate`: up from a little below.
    Translate,
    /// `scale`: from a little small.
    Scale,
}

impl MotionProperty {
    /// The property in the target API.
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Opacity => "ui::Property::Opacity",
            Self::Translate => "ui::Property::TranslateY",
            Self::Scale => "ui::Property::Scale",
        }
    }

    /// Its CSS name, as an outline shows it.
    #[must_use]
    pub const fn css(self) -> &'static str {
        match self {
            Self::Opacity => "opacity",
            Self::Translate => "transform",
            Self::Scale => "scale",
        }
    }
}

/// `transition: opacity 200ms ease-out 50ms` (contract v23).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Transition {
    /// What moves.
    pub property: MotionProperty,
    /// How long, in milliseconds.
    pub duration: u32,
    /// After how long, in milliseconds.
    pub delay: u32,
    /// How it is paced.
    pub easing: Easing,
}

/// One of the runtime's animations, by name (contract v23).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnimationName {
    /// `spin`: an icon turning.
    Spin,
    /// `pulse`: growing a little and back.
    Pulse,
    /// `fade`: fading in.
    Fade,
    /// `indeterminate`: a progress bar's segment sweeping.
    Indeterminate,
    /// `shimmer`: a light band crossing.
    Shimmer,
}

impl AnimationName {
    /// Each one's name.
    pub const NAMES: [&'static str; 5] = ["spin", "pulse", "fade", "indeterminate", "shimmer"];

    /// The one named `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "spin" => Some(Self::Spin),
            "pulse" => Some(Self::Pulse),
            "fade" => Some(Self::Fade),
            "indeterminate" => Some(Self::Indeterminate),
            "shimmer" => Some(Self::Shimmer),
            _ => None,
        }
    }

    /// Its name.
    #[must_use]
    pub const fn css(self) -> &'static str {
        match self {
            Self::Spin => "spin",
            Self::Pulse => "pulse",
            Self::Fade => "fade",
            Self::Indeterminate => "indeterminate",
            Self::Shimmer => "shimmer",
        }
    }
}

/// `animation: spin 1s linear infinite` (contract v23): one of the
/// runtime's animations with its timing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NamedAnimation {
    /// Which.
    pub name: AnimationName,
    /// How long a run is, in milliseconds.
    pub duration: u32,
    /// After how long it starts, in milliseconds.
    pub delay: u32,
    /// How it is paced.
    pub easing: Easing,
    /// How many runs; `None` for `infinite`.
    pub repeat: Option<u16>,
    /// `alternate`: every second run backwards.
    pub alternate: bool,
    /// `forwards` or `both`: its last value kept.
    pub keep: bool,
}

/// One of the theme's shadows, by name: `boxShadow: "medium"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Elevation {
    /// `low`: a card resting on the screen.
    Low,
    /// `medium`: a menu or popover.
    Medium,
    /// `high`: a dialog.
    High,
}

impl Elevation {
    /// Each one's name, in order.
    pub const NAMES: [&'static str; 3] = ["low", "medium", "high"];

    /// The one named `name`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "low" => Some(Self::Low),
            "medium" => Some(Self::Medium),
            "high" => Some(Self::High),
            _ => None,
        }
    }

    /// Its name.
    #[must_use]
    pub const fn tsx(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    /// Its name in the target API (`ui::Elevation::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

/// A gradient between two of the theme's colours (contract v22): along a
/// line at `angle` degrees, or radial when `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gradient {
    /// Its angle in degrees (180, top to bottom); `None` for radial.
    pub angle: Option<u16>,
    /// Its first colour.
    pub from: Colour,
    /// Its last colour.
    pub to: Colour,
}

impl Look {
    /// Whether it names nothing: the kind's own look.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.fill.is_none()
            && self.border.is_none()
            && self.border_width.is_none()
            && self.radius.is_none()
            && self.text.is_none()
            && self.shadow.is_none()
            && self.gradient.is_none()
            && self.opacity.is_none()
            && self.transition.is_none()
            && self.animation.is_none()
    }
}

/// The widest border a style may name, in layout units.
pub const MAX_BORDER_WIDTH: u16 = 16;

/// The largest corner radius a style may name, in layout units.
pub const MAX_RADIUS: u16 = 64;

/// One of the theme's spacing tokens: `space.xs` to `space.xl`, from
/// `import { space } from "nowayos:kit"`. NoWayOS measures it against
/// the theme when it lays the screen out - 4, 8, 16, 24 and 32 layout units
/// in the light theme, half again in the large-text theme - where a number
/// or a screen's own `const` stays fixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    /// `space.xs`.
    Xs,
    /// `space.sm`.
    Sm,
    /// `space.md`.
    Md,
    /// `space.lg`.
    Lg,
    /// `space.xl`.
    Xl,
}

impl Space {
    /// The token `space.NAME` names: `xs`, `sm`, `md`, `lg` or `xl`.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "xs" => Some(Self::Xs),
            "sm" => Some(Self::Sm),
            "md" => Some(Self::Md),
            "lg" => Some(Self::Lg),
            "xl" => Some(Self::Xl),
            _ => None,
        }
    }

    /// Every token's name, smallest first.
    pub const NAMES: [&'static str; 5] = ["xs", "sm", "md", "lg", "xl"];

    /// Its name after `space.`: `xs` to `xl`.
    #[must_use]
    pub const fn tsx(self) -> &'static str {
        match self {
            Self::Xs => "xs",
            Self::Sm => "sm",
            Self::Md => "md",
            Self::Lg => "lg",
            Self::Xl => "xl",
        }
    }

    /// Its name in the target API (`ui::Space::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Xs => "Xs",
            Self::Sm => "Sm",
            Self::Md => "Md",
            Self::Lg => "Lg",
            Self::Xl => "Xl",
        }
    }
}

/// The icons `<Icon name="…">` may name: NoWayOS's,
/// `nowayos_ui::icons::ALL`, by Lucide name, in its order. Screen Compiler keeps
/// its own copy, so its crates never depend on NoWayOS; `make
/// contract-check` holds the two the same.
pub const ICON_NAMES: [&str; 6] = [
    "clipboard-list",
    "fuel",
    "house",
    "message-square",
    "shield-check",
    "user-check",
];

/// The most lines a `<Text>` may wrap onto: NoWayOS's
/// `nowayos_ui::MAX_LINES`.
pub const MAX_LINES: u8 = 8;

/// How a text is set: its role, its weight, the most lines it may wrap
/// onto and where its lines sit. Body text, Regular, on one line, against
/// the start, by default - what `<Text>` without a `role`, `lines` or
/// `align` is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextStyle {
    /// Its role in the type scale.
    pub role: TextRole,
    /// Its weight.
    pub weight: Weight,
    /// The most lines its text may wrap onto, from 1 to [`MAX_LINES`]:
    /// `<Text lines={3}>`. 1, the default, keeps it on one line.
    pub lines: u8,
    /// Where its lines sit across the node: `<Text align="center">`.
    pub align: TextAlign,
    /// Its colour, one of the theme's: a style's `color`.
    pub colour: Option<Colour>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::BODY
    }
}

impl TextStyle {
    /// Body text, Regular.
    pub const BODY: Self = Self::new(TextRole::Body);

    /// `role` in its own weight: Bold for display, title and heading,
    /// Regular for the rest.
    #[must_use]
    pub const fn new(role: TextRole) -> Self {
        let weight = if role.bold_only() {
            Weight::Bold
        } else {
            Weight::Regular
        };
        Self {
            role,
            weight,
            lines: 1,
            align: TextAlign::Start,
            colour: None,
        }
    }

    /// The same, in Bold.
    #[must_use]
    pub const fn bold(self) -> Self {
        Self {
            weight: Weight::Bold,
            ..self
        }
    }

    /// The same, wrapping onto at most `lines` lines. Validation
    /// refuses a number outside 1 to [`MAX_LINES`].
    #[must_use]
    pub const fn lines(self, lines: u8) -> Self {
        Self { lines, ..self }
    }

    /// The same, its lines set `align` across the node.
    #[must_use]
    pub const fn align(self, align: TextAlign) -> Self {
        Self { align, ..self }
    }

    /// The same, in the theme's `colour`.
    #[must_use]
    pub const fn colour(self, colour: Colour) -> Self {
        Self {
            colour: Some(colour),
            ..self
        }
    }
}

/// How a button is drawn: the TSX `variant` prop of `<Button>`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ButtonVariant {
    /// The one main action: filled with the brand colour. The default.
    #[default]
    Primary,
    /// Another action: outlined.
    Secondary,
    /// A minor action, such as Cancel: its label alone.
    Quiet,
}

impl ButtonVariant {
    /// Every variant.
    pub const ALL: [Self; 3] = [Self::Primary, Self::Secondary, Self::Quiet];

    /// Every variant's TSX `variant` value.
    pub const NAMES: [&'static str; 3] = ["primary", "secondary", "quiet"];

    /// The variant a TSX `variant` value names: `primary`, `secondary` or
    /// `quiet`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|variant| variant.tsx() == value)
    }

    /// Its TSX `variant` value.
    #[must_use]
    pub const fn tsx(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
            Self::Quiet => "quiet",
        }
    }

    /// Its name in the target API (`ui::ButtonVariant::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Secondary => "Secondary",
            Self::Quiet => "Quiet",
        }
    }
}

/// What a notice tells: the TSX `tone` prop of `<Notice>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tone {
    /// Something done or ready.
    Success,
    /// Something to note.
    Caution,
    /// Something that needs care.
    Warning,
}

impl Tone {
    /// Every tone.
    pub const ALL: [Self; 3] = [Self::Success, Self::Caution, Self::Warning];

    /// Every tone's TSX `tone` value.
    pub const NAMES: [&'static str; 3] = ["success", "caution", "warning"];

    /// The tone a TSX `tone` value names: `success`, `caution` or
    /// `warning`.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tone| tone.tsx() == value)
    }

    /// Its TSX `tone` value.
    #[must_use]
    pub const fn tsx(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Caution => "caution",
            Self::Warning => "warning",
        }
    }

    /// Its name in the target API (`ui::Tone::…`).
    #[must_use]
    pub const fn rust(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::Caution => "Caution",
            Self::Warning => "Warning",
        }
    }
}

/// The surface a container is drawn on: `<Card>`, `<HeaderBar>` or
/// `<Notice tone="…">`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Surface {
    /// A card: a `<Card>`, its children in a column.
    Card,
    /// A header bar: a `<HeaderBar>`, its children in a row.
    Header,
    /// A notice in a tone: a `<Notice>`, its children in a column.
    Notice(Tone),
}

impl Surface {
    /// The surface in the target API: `ui::Surface::Card`,
    /// `ui::Surface::Notice(ui::Tone::Success)`.
    #[must_use]
    pub fn rust(self) -> String {
        match self {
            Self::Card => "ui::Surface::Card".to_owned(),
            Self::Header => "ui::Surface::Header".to_owned(),
            Self::Notice(tone) => format!("ui::Surface::Notice(ui::Tone::{})", tone.rust()),
        }
    }

    /// The surface in an outline: `card`, `header`, `notice-success`.
    #[must_use]
    pub fn outline(self) -> String {
        match self {
            Self::Card => "card".to_owned(),
            Self::Header => "header".to_owned(),
            Self::Notice(tone) => format!("notice-{}", tone.tsx()),
        }
    }
}

/// Whether the body role's atlases - every role's but display's - draw
/// `c`: printable ASCII and Latin-1's U+00A0 to U+00FF, except U+00AD SOFT
/// HYPHEN, which shows nothing. NoWayOS's `nowayos_ui::text::is_printable`.
#[must_use]
pub const fn is_printable(c: char) -> bool {
    matches!(c, ' '..='~' | '\u{A0}'..='\u{FF}') && c != '\u{AD}'
}

/// Whether the display role's atlases draw `c`: the digits, the capitals,
/// the space and `# % + , - . / : £` - what a queue number, an amount, a
/// time or a reference needs. NoWayOS's `nowayos_ui::text::is_display`.
#[must_use]
pub const fn is_display(c: char) -> bool {
    matches!(c, ' ' | '#' | '%' | '+'..=':' | 'A'..='Z' | '\u{A3}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_role_parses_from_its_tsx_value_and_nothing_else_does() {
        for role in TextRole::ALL {
            assert_eq!(TextRole::parse(role.tsx()), Some(role));
            assert_eq!(role.to_string(), role.tsx());
        }
        for value in ["", "Title", "headline", "small", "body "] {
            assert_eq!(TextRole::parse(value), None, "{value:?}");
        }
        assert_eq!(Weight::parse("bold"), Some(Weight::Bold));
        assert_eq!(Weight::parse("regular"), Some(Weight::Regular));
        for value in ["Bold", "700", "normal", ""] {
            assert_eq!(Weight::parse(value), None, "{value:?}");
        }
    }

    #[test]
    fn display_title_and_heading_are_bold_and_the_rest_regular() {
        assert_eq!(TextStyle::default(), TextStyle::BODY);
        for role in TextRole::ALL {
            let weight = TextStyle::new(role).weight;
            assert_eq!(weight == Weight::Bold, role.bold_only(), "{role}");
            assert_eq!(TextStyle::new(role).bold().weight, Weight::Bold);
        }
    }

    #[test]
    fn every_variant_and_tone_parses_from_its_tsx_value_and_nothing_else_does() {
        for variant in ButtonVariant::ALL {
            assert_eq!(ButtonVariant::parse(variant.tsx()), Some(variant));
        }
        for tone in Tone::ALL {
            assert_eq!(Tone::parse(tone.tsx()), Some(tone));
        }
        for value in ["", "Primary", "ghost", "danger", "error", "info"] {
            assert_eq!(ButtonVariant::parse(value), None, "{value:?}");
            assert_eq!(Tone::parse(value), None, "{value:?}");
        }
        assert_eq!(ButtonVariant::default(), ButtonVariant::Primary);
        assert_eq!(
            Surface::Notice(Tone::Caution).rust(),
            "ui::Surface::Notice(ui::Tone::Caution)"
        );
        assert_eq!(Surface::Notice(Tone::Warning).outline(), "notice-warning");
    }

    #[test]
    fn each_name_table_lists_exactly_the_values_that_parse() {
        let roles: Vec<&str> = TextRole::ALL.iter().map(|r| r.tsx()).collect();
        assert_eq!(roles, TextRole::NAMES);
        let variants: Vec<&str> = ButtonVariant::ALL.iter().map(|v| v.tsx()).collect();
        assert_eq!(variants, ButtonVariant::NAMES);
        let tones: Vec<&str> = Tone::ALL.iter().map(|t| t.tsx()).collect();
        assert_eq!(tones, Tone::NAMES);
        assert!(Weight::NAMES.iter().all(|w| Weight::parse(w).is_some()));
        assert!(
            TextAlign::STYLE_NAMES
                .iter()
                .all(|a| TextAlign::parse_style(a).is_some())
        );
        assert!(
            TextAlign::NAMES
                .iter()
                .all(|a| TextAlign::parse(a).is_some())
        );
        for value in ["", "Bold", "medium", "justify", "middle"] {
            assert_eq!(Weight::parse(value), None, "{value:?}");
            assert_eq!(TextAlign::parse_style(value), None, "{value:?}");
        }
    }

    #[test]
    fn the_display_set_is_a_part_of_the_printable_set() {
        let display: String = ('\0'..='\u{FF}').filter(|&c| is_display(c)).collect();
        assert_eq!(
            display,
            " #%+,-./0123456789:ABCDEFGHIJKLMNOPQRSTUVWXYZ\u{a3}"
        );
        assert!(display.chars().all(is_printable));
        assert!(!is_printable('\u{AD}') && !is_printable('\u{7F}') && !is_printable('\u{100}'));
        assert!(is_printable('\u{A0}') && is_printable('\u{FF}') && is_printable('~'));
        for role in TextRole::ALL {
            assert_eq!(role.draws('a'), role != TextRole::Display, "{role}");
            assert!(role.draws('7') && !role.draws('\u{20AC}'), "{role}");
        }
    }
}

// SPDX-License-Identifier: MIT
//! Draws a screen Screen Compiler generated: built on a real `nowayos-ui`
//! tree, laid out and drawn by NoWayOS's renderer, as the device draws it.
//!
//! One function, [`frame`], and what it is asked to show, a [`View`]. The
//! contract check's tests draw every fixture through it and compare each
//! frame with its approved image; `make preview` draws a developer's own
//! screen through it. So a preview is what the contract check approves.

use std::error::Error;
use std::fmt;

use nowayos_display::Rect;
use nowayos_render_eg::{Theme, render};
use nowayos_testkit::Framebuffer;
use nowayos_ui::{
    FieldId, GroupId, InputMode, LayerId, ListId, NodeId, NodeKind, SlotId, SwitchId, Tree, UiError,
};

/// Nodes a screen may use.
pub const CAPACITY: usize = 128;
/// Bytes of long text a screen may hold (contract v16).
pub const TEXT: usize = 16_384;
/// The tree a screen is built on.
pub type Screen = Tree<CAPACITY, TEXT>;

/// The width a screen is drawn at unless a view says otherwise: the
/// Raspberry Pi Touch Display's.
pub const WIDTH: u32 = 800;
/// The height that goes with [`WIDTH`].
pub const HEIGHT: u32 = 480;

/// What a long text shows when no text is given for its slot (contract
/// v16): a synthetic article of 10 KB, `fixtures/article.txt`.
const ARTICLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/article.txt"
));

/// What a typed-in text area holds in a view with `errors` (contract v19): a
/// line long enough to wrap, and a second paragraph.
const TYPED: &str = "Text a person typed, long enough to wrap onto a second line at the width of the area.\nAnd a second paragraph.";

/// How a screen is shown: its size and theme, and what the application
/// would have set by then.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    /// The viewport's width, in pixels.
    pub width: u32,
    /// The viewport's height, in pixels.
    pub height: u32,
    /// The large-text, high-contrast theme, whose spacing tokens measure
    /// half again; otherwise the light theme.
    pub large: bool,
    /// Groups made visible, by number.
    pub groups: Vec<u16>,
    /// Slots filled, by number: with the text given, or with none a sample -
    /// "Text from the application, N"; for a slot in the display role, which
    /// draws only digits, capitals and a few signs, "A00N"; for a long text,
    /// an article; for a progress bar, three fifths (given a text, "DONE/OF").
    /// With any slot filled, a virtual list (contract v18) holds a thousand
    /// items.
    pub slots: Vec<(u16, Option<String>)>,
    /// Switches set to a case (contract v13): the switch's number and the
    /// case's.
    pub cases: Vec<(u16, u8)>,
    /// The scroll view scrolled as far as it goes.
    pub scrolled: bool,
    /// The first text input in this mode focused - scrolled into view - so
    /// the on-screen keyboard shows that mode's layout.
    pub typing: Option<InputMode>,
    /// Every field's error set to "Error from the application", and every
    /// text area (contract v19) holding two paragraphs.
    pub errors: bool,
    /// One layer open over the screen (contract v20), by number.
    pub layer: Option<u16>,
}

impl Default for View {
    /// The screen as it starts: 800 x 480, the light theme, nothing set.
    fn default() -> Self {
        Self {
            width: WIDTH,
            height: HEIGHT,
            large: false,
            groups: Vec::new(),
            slots: Vec::new(),
            cases: Vec::new(),
            scrolled: false,
            typing: None,
            errors: false,
            layer: None,
        }
    }
}

/// Why a screen could not be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawError(String);

impl fmt::Display for DrawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for DrawError {}

impl From<UiError> for DrawError {
    fn from(error: UiError) -> Self {
        Self(error.to_string())
    }
}

/// Builds a screen on a real tree, makes it the root, sets what `view`
/// names, lays it out in the view's viewport with its theme's font and draws
/// it with that theme.
///
/// # Errors
///
/// A [`DrawError`] if the screen does not build or lay out, if the view
/// names something the screen has not got, or if the frame cannot be made.
pub fn frame(
    build: impl FnOnce(&mut Screen) -> Result<NodeId, UiError>,
    view: &View,
) -> Result<Framebuffer, DrawError> {
    let theme = if view.large {
        Theme::high_contrast()
    } else {
        Theme::light()
    };
    let mut tree = Screen::new();
    let root = build(&mut tree)?;
    tree.set_root(root)?;
    for &group in &view.groups {
        tree.set_visible(GroupId::new(group), true)?;
    }
    for &(switch, when) in &view.cases {
        tree.set_case(SwitchId::new(switch), Some(when))?;
    }
    if !view.slots.is_empty() {
        let lists: Vec<ListId> = tree
            .scroll_views()
            .filter_map(|id| match tree.get(id).map(|node| node.kind()) {
                Some(NodeKind::Scroll {
                    rows: Some(rows), ..
                }) => Some(rows.list),
                _ => None,
            })
            .collect();
        for list in lists {
            tree.set_list_count(list, 1_000)?;
        }
    }
    for (slot, text) in &view.slots {
        fill(&mut tree, *slot, text.as_deref())?;
    }
    if view.errors {
        let fields: Vec<FieldId> = tree
            .paint_order()
            .filter_map(|(_, node)| match node.kind() {
                NodeKind::FieldError { field, .. } => Some(*field),
                _ => None,
            })
            .collect();
        for field in fields {
            tree.set_field_error(field, "Error from the application")?;
        }
    }
    tree.layout(Rect::new(0, 0, view.width, view.height), &theme.metrics())?;
    if view.errors {
        let areas: Vec<FieldId> = tree
            .paint_order()
            .filter_map(|(_, node)| match node.kind() {
                NodeKind::TextArea { field, .. } => Some(*field),
                _ => None,
            })
            .collect();
        for field in areas {
            tree.set_value(field, TYPED)?;
        }
    }
    if view.scrolled {
        tree.scroll_to(u32::MAX)?;
    }
    if let Some(layer) = view.layer {
        tree.open(LayerId::new(layer))?;
    }
    if let Some(typing) = view.typing {
        let input = tree
            .paint_order()
            .find(|(_, node)| {
                matches!(node.kind(), NodeKind::TextInput { mode, .. } if *mode == typing)
            })
            .map(|(id, _)| id);
        tree.set_focus(input)?;
    }
    let mut frame =
        Framebuffer::new(view.width, view.height).map_err(|error| DrawError(error.to_string()))?;
    nowayos_testkit::glyph_store!(glyphs);
    render(&tree, &mut frame, &theme, &mut glyphs)
        .map_err(|error| DrawError(format!("drawing: {error:?}")))?;
    Ok(frame)
}

/// Fills one slot: with `text`, or with a sample that suits it.
fn fill(tree: &mut Screen, slot: u16, text: Option<&str>) -> Result<(), DrawError> {
    let id = SlotId::new(slot);
    let Some(text) = text else {
        // A long text (contract v16) shows a 10 KB article.
        let long = tree
            .find_slot(id)
            .and_then(|node| tree.get(node))
            .and_then(|node| tree.long_text_of(node))
            .is_some();
        if long {
            // As much of the article as the slot holds.
            let mut text = ARTICLE;
            loop {
                match tree.set_slot(id, text) {
                    Err(UiError::TextTooLong(_)) if !text.is_empty() => {
                        let mut end = text.len() / 2;
                        while !text.is_char_boundary(end) {
                            end = end.saturating_sub(1);
                        }
                        text = text.get(..end).unwrap_or_default();
                    }
                    other => return Ok(other?),
                }
            }
        }
        return match tree.set_slot(id, &format!("Text from the application, {slot}")) {
            Err(UiError::NoGlyph | UiError::TextTooLong(_)) => {
                Ok(tree.set_slot(id, &format!("A{slot:03}"))?)
            }
            // A progress bar, shown three fifths full.
            Err(UiError::UnknownSlot(_)) if tree.find_progress(id).is_some() => {
                Ok(tree.set_progress(id, 3, 5)?)
            }
            other => Ok(other?),
        };
    };
    match tree.set_slot(id, text) {
        Err(UiError::UnknownSlot(_)) if tree.find_progress(id).is_some() => {
            let parts = text.split_once('/').and_then(|(done, whole)| {
                Some((done.trim().parse().ok()?, whole.trim().parse().ok()?))
            });
            let Some((done, whole)) = parts else {
                return Err(DrawError(format!(
                    "a progress bar takes DONE/OF, two whole numbers, not {text:?}"
                )));
            };
            Ok(tree.set_progress(id, done, whole)?)
        }
        other => Ok(other?),
    }
}

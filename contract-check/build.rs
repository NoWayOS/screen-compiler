// SPDX-License-Identifier: MIT
//! Writes `screens.rs` into `OUT_DIR`: one module per file of generated Rust,
//! holding the file exactly as Screen Compiler wrote it, and one test per screen in
//! it that builds the screen on a real nowayos-ui tree, lays it out and draws
//! it with NoWayOS's renderer. The files are every checked-in snapshot, and
//! every fixture `make contract-check` converted afresh into the directory
//! `SCREEN_COMPILER_CONVERTED` names. Each converted screen's frame must match its
//! approved image, `golden/FIXTURE/SCREEN.png`, pixel for pixel, and
//! every approved image must belong to a converted screen. A converted screen
//! with groups, slots (subset v1) or field errors (contract v12)
//! is drawn a second time as the application would show it with every
//! group visible, every slot filled, every field's error set and every
//! text area (contract v19) holding two paragraphs, and that frame must
//! match `golden/FIXTURE/SCREEN.shown.png`. A converted
//! screen that names one of the theme's spacing tokens (contract v7)
//! or colours (contract v8) is drawn a third time, in the large-text,
//! high-contrast theme, which spaces half again as much and has colours of
//! its own: that frame must match `golden/FIXTURE/SCREEN.large.png`. A
//! screen that names a theme colour must also keep every text it draws at
//! WCAG's 4.5:1 against what lies beneath it, in each theme. A converted
//! screen with a scroll view (contract v9) is drawn once more,
//! every group visible and slot filled and the view scrolled to its end:
//! `golden/FIXTURE/SCREEN.scrolled.png`. A converted screen with the
//! on-screen keyboard (contract v11) is drawn once for each mode its
//! text inputs take, the first input in that mode focused, so the keyboard
//! shows that mode's layout: `golden/FIXTURE/SCREEN.keys-MODE.png`, MODE
//! `text`, `digits` or `email`. A converted screen with switches (contract
//! v13) is drawn once for each case of each, every group visible and slot
//! filled and that switch set to that case:
//! `golden/FIXTURE/SCREEN.case-SWITCH-CASE.png`.
//!
//! Finding the screens reads the generated code's fixed shape: each screen is
//! a top-level `pub mod NAME {` holding `pub fn build<B: ui::UiBuilder>(`.

use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The snapshots of generated Rust, from this crate's directory.
const SNAPSHOTS: &str = "../crates/screen-compiler-codegen-rust/tests/snapshots";

/// One file of generated Rust: the module that holds it, its path, its text,
/// and - for a fixture converted afresh - the fixture's name, which names the
/// directory of its screens' approved images.
struct Generated {
    module: String,
    path: PathBuf,
    source: String,
    fixture: Option<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let snapshots = manifest.join(SNAPSHOTS);
    println!("cargo::rerun-if-changed={}", snapshots.display());
    println!("cargo::rerun-if-env-changed=SCREEN_COMPILER_CONVERTED");
    let mut files = generated(&snapshots, "snapshot")?;
    if let Some(converted) = env::var_os("SCREEN_COMPILER_CONVERTED") {
        let converted = PathBuf::from(converted);
        println!("cargo::rerun-if-changed={}", converted.display());
        let mut fresh = generated(&converted, "converted")?;
        for file in &mut fresh {
            file.fixture = Some(fixture_name(&file.path)?);
        }
        files.extend(fresh);
    } else {
        println!(
            "cargo::warning=SCREEN_COMPILER_CONVERTED is not set, so no screen is compared with its approved image: run make contract-check"
        );
    }
    if files.is_empty() {
        return Err(format!("no generated Rust found in {}", snapshots.display()).into());
    }

    let mut out = String::new();
    let mut tests = String::new();
    // The approved image of each converted screen, relative to golden/.
    let mut images = Vec::new();
    // Every generated name, so two files that map to the same one are named.
    let mut names: HashSet<String> = HashSet::new();
    writeln!(
        out,
        "/// Nodes a screen may use: more than any fixture needs."
    )?;
    writeln!(out, "pub const CAPACITY: usize = 128;")?;
    writeln!(
        out,
        "/// Bytes of long text a screen may hold (contract v16): more than any fixture needs."
    )?;
    writeln!(out, "pub const TEXT: usize = 16_384;")?;
    for file in &files {
        let screens = screens(&file.source);
        if screens.is_empty() {
            return Err(format!(
                "{}: no screen found (no `pub fn build`)",
                file.path.display()
            )
            .into());
        }
        let path = file.path.display().to_string();
        if !names.insert(file.module.clone()) {
            return Err(format!(
                "{path}: its module name, {}, is taken by another generated file: rename one",
                file.module
            )
            .into());
        }
        writeln!(out, "/// `{}`, built against the real nowayos-ui.", path)?;
        writeln!(out, "pub mod {} {{", file.module)?;
        writeln!(out, "    include!({path:?});")?;
        writeln!(out, "}}")?;
        let module = &file.module;
        for screen in screens {
            if !names.insert(format!("{module}_{screen}")) {
                return Err(format!("{path}: screen {screen} gives the name {module}_{screen}, which another generated file already gives: rename one").into());
            }
            // A concrete instance in the library itself, so that every target this
            // crate is built for - aarch64-unknown-none too - compiles the screen
            // against the real Tree, not only the host's tests.
            writeln!(
                out,
                "/// `{module}::{screen}` on a real `nowayos_ui::Tree`."
            )?;
            writeln!(out, "///\n/// # Errors\n///\n/// The tree's error.")?;
            writeln!(
                out,
                "pub fn build_{module}_{screen}(tree: &mut nowayos_ui::Tree<CAPACITY, TEXT>) -> Result<nowayos_ui::NodeId, nowayos_ui::UiError> {{"
            )?;
            writeln!(
                out,
                "    {module}::{screen}::build(tree).map(|nodes| nodes.root)"
            )?;
            writeln!(out, "}}")?;
            writeln!(tests, "    #[test]")?;
            writeln!(tests, "    fn {module}_{screen}() {{")?;
            match &file.fixture {
                Some(fixture) => {
                    let image = format!("{fixture}/{screen}.png");
                    writeln!(
                        tests,
                        "        matches_approved(super::build_{module}_{screen}, {image:?});"
                    )?;
                    images.push(image);
                }
                None => writeln!(tests, "        frame(super::build_{module}_{screen});")?,
            }
            writeln!(tests, "    }}")?;
            // In the large-text theme too, where the theme's spacing tokens
            // measure half again.
            if let Some(fixture) = &file.fixture
                && (screen_names(&file.source, &screen, "ui::Space::")
                    || screen_names(&file.source, &screen, "ui::Colour::"))
            {
                let image = format!("{fixture}/{screen}.large.png");
                writeln!(tests, "    #[test]")?;
                writeln!(tests, "    fn {module}_{screen}_large() {{")?;
                writeln!(
                    tests,
                    "        matches_approved_large(super::build_{module}_{screen}, {image:?});"
                )?;
                writeln!(tests, "    }}")?;
                images.push(image);
            }
            // Its own colours readable in every theme.
            if screen_names(&file.source, &screen, "ui::Colour::") {
                writeln!(tests, "    #[test]")?;
                writeln!(tests, "    fn {module}_{screen}_contrast() {{")?;
                writeln!(
                    tests,
                    "        contrast_holds(super::build_{module}_{screen});"
                )?;
                writeln!(tests, "    }}")?;
            }
            let groups = numbers(&file.source, &screen, "GroupId");
            let slots = numbers(&file.source, &screen, "SlotId");
            // Scrolled to its end.
            if let Some(fixture) = &file.fixture
                && screen_names(&file.source, &screen, "b.scroll_view(")
            {
                let image = format!("{fixture}/{screen}.scrolled.png");
                writeln!(tests, "    #[test]")?;
                writeln!(tests, "    fn {module}_{screen}_scrolled() {{")?;
                writeln!(
                    tests,
                    "        matches_approved_scrolled(super::build_{module}_{screen}, &{groups:?}, &{slots:?}, {image:?});"
                )?;
                writeln!(tests, "    }}")?;
                images.push(image);
            }
            // Each layout of the keyboard, typing for the first input in
            // its mode.
            if let Some(fixture) = &file.fixture
                && screen_names(&file.source, &screen, "b.keyboard(")
            {
                for (mode, variant) in input_modes(&file.source, &screen) {
                    let image = format!("{fixture}/{screen}.keys-{mode}.png");
                    writeln!(tests, "    #[test]")?;
                    writeln!(tests, "    fn {module}_{screen}_keys_{mode}() {{")?;
                    writeln!(
                        tests,
                        "        matches_approved_keys(super::build_{module}_{screen}, nowayos_ui::InputMode::{variant}, {image:?});"
                    )?;
                    writeln!(tests, "    }}")?;
                    images.push(image);
                }
            }
            // Each layer, open (contract v20).
            for (number, name) in layers(&file.source, &screen) {
                writeln!(tests, "    #[test]")?;
                writeln!(
                    tests,
                    "    fn {module}_{screen}_layer_{}() {{",
                    name.replace('-', "_")
                )?;
                let build = format!("super::build_{module}_{screen}");
                match &file.fixture {
                    Some(fixture) => {
                        let image = format!("{fixture}/{screen}.layer-{name}.png");
                        writeln!(
                            tests,
                            "        matches_approved_layer({build}, &{groups:?}, &{slots:?}, {number}, {image:?});"
                        )?;
                        images.push(image);
                    }
                    None => writeln!(
                        tests,
                        "        frame_in(false, {build}, &{groups:?}, &{slots:?}, None, false, None, true, Some({number}));"
                    )?,
                }
                writeln!(tests, "    }}")?;
            }
            // Each case of each switch.
            for Switch {
                number: switch,
                name,
                cases,
            } in switches(&file.source, &screen)
            {
                for (when, case) in cases {
                    writeln!(tests, "    #[test]")?;
                    writeln!(tests, "    fn {module}_{screen}_case_{name}_{case}() {{")?;
                    let build = format!("super::build_{module}_{screen}");
                    match &file.fixture {
                        Some(fixture) => {
                            let image = format!("{fixture}/{screen}.case-{name}-{case}.png");
                            writeln!(
                                tests,
                                "        matches_approved_case({build}, &{groups:?}, &{slots:?}, ({switch}, {when}), {image:?});"
                            )?;
                            images.push(image);
                        }
                        None => writeln!(
                            tests,
                            "        frame_case({build}, &{groups:?}, &{slots:?}, ({switch}, {when}));"
                        )?,
                    }
                    writeln!(tests, "    }}")?;
                }
            }
            if groups.is_empty()
                && slots.is_empty()
                && !screen_names(&file.source, &screen, "b.field_error(")
            {
                continue;
            }
            writeln!(tests, "    #[test]")?;
            writeln!(tests, "    fn {module}_{screen}_shown() {{")?;
            let build = format!("super::build_{module}_{screen}");
            match &file.fixture {
                Some(fixture) => {
                    let image = format!("{fixture}/{screen}.shown.png");
                    writeln!(
                        tests,
                        "        matches_approved_shown({build}, &{groups:?}, &{slots:?}, {image:?});"
                    )?;
                    images.push(image);
                }
                None => writeln!(
                    tests,
                    "        frame_shown({build}, &{groups:?}, &{slots:?});"
                )?,
            }
            writeln!(tests, "    }}")?;
        }
    }
    out.push_str(RUN);
    if !images.is_empty() {
        out.push_str(RUN_IMAGES);
        writeln!(tests, "    #[test]")?;
        writeln!(tests, "    fn every_approved_image_has_a_screen() {{")?;
        writeln!(tests, "        only_these_images(&{images:?});")?;
        writeln!(tests, "    }}")?;
    }
    out.push_str(&tests);
    writeln!(out, "}}")?;

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out_dir.join("screens.rs"), out)?;
    Ok(())
}

/// The start of the generated test module: drawing a screen. The image
/// helpers and the tests themselves follow.
const RUN: &str = r#"
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a screen that fails to build, lay out, draw or match fails the test"
)]
mod run {
    extern crate std;

    use nowayos_display::Rect;
    use nowayos_render_eg::{Theme, render};
    use nowayos_testkit::Framebuffer;
    use nowayos_ui::{GroupId, NodeId, SlotId, SwitchId, Tree, UiError};

    use super::{CAPACITY, TEXT};

    /// What a long text shows in a screen's shown frame (contract v16): a
    /// synthetic article of 10 KB, `fixtures/article.txt`.
    const ARTICLE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/article.txt"));

    /// The viewport every screen is laid out and drawn in: 800 x 480, the
    /// Raspberry Pi Touch Display's resolution.
    const WIDTH: u32 = 800;
    const HEIGHT: u32 = 480;

    /// Builds a screen on a real tree, makes it the root, lays it out in the
    /// viewport with the light theme's font and draws it with that theme.
    fn frame(build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>) -> Framebuffer {
        frame_in(false, build, &[], &[], None, false, None, false, None)
    }

    /// As `frame`, in the large-text, high-contrast theme, whose spacing
    /// tokens measure half again.
    fn frame_large(build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>) -> Framebuffer {
        frame_in(true, build, &[], &[], None, false, None, false, None)
    }

    /// As `frame`, with `groups` made visible and each of `slots` given the
    /// text "Text from the application, N" first - or, for a slot in the
    /// display role, which draws only digits, capitals and a few signs,
    /// "A00N"; a progress bar three fifths; and every
    /// field's error set: the screen as the application would show
    /// it with every condition holding.
    fn frame_shown(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        groups: &[u16],
        slots: &[u16],
    ) -> Framebuffer {
        frame_in(false, build, groups, slots, None, false, None, true, None)
    }

    /// As `frame_shown`, with `case`'s switch set to its case.
    fn frame_case(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        groups: &[u16],
        slots: &[u16],
        case: (u16, u8),
    ) -> Framebuffer {
        frame_in(false, build, groups, slots, Some(case), false, None, true, None)
    }

    /// Every text a screen draws - in the colours its style names, or its
    /// roles' own - stands out from what lies beneath it as WCAG 2 asks of
    /// text, 4.5:1 as the screen shows it, in the light and the large-text
    /// theme.
    fn contrast_holds(build: fn(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>) {
        use nowayos_render_eg::contrast::{Need, as_shown, ratio, text_colours};
        for (name, theme) in Theme::all() {
            let mut tree = Tree::<CAPACITY, TEXT>::new();
            let root = build(&mut tree).unwrap();
            tree.set_root(root).unwrap();
            tree.layout(Rect::new(0, 0, WIDTH, HEIGHT), &theme.metrics())
                .unwrap();
            for (id, text, ground) in text_colours(&tree, &theme) {
                let found = ratio(as_shown(text), as_shown(ground));
                assert!(
                    found >= Need::Text.minimum(),
                    "{name} theme: node {id:?}'s text is {found:.2}:1 against what lies beneath it"
                );
            }
        }
    }

    /// As `frame_shown`, in the light theme, or the large-text,
    /// high-contrast theme if `large`; given `case`, that switch set to
    /// that case (contract v13); if `scrolled`, its scroll view
    /// scrolled as far as it goes; given `typing`, the first text
    /// input in that mode focused - scrolled into view - so the on-screen
    /// keyboard shows its layout; and, if `errors`, every field's
    /// error set to "Error from the application".
    #[allow(
        clippy::too_many_arguments,
        reason = "test code: each frame helper above names the arguments it sets"
    )]
    fn frame_in(
        large: bool,
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        groups: &[u16],
        slots: &[u16],
        case: Option<(u16, u8)>,
        scrolled: bool,
        typing: Option<nowayos_ui::InputMode>,
        errors: bool,
        layer: Option<u16>,
    ) -> Framebuffer {
        let theme = if large {
            Theme::high_contrast()
        } else {
            Theme::light()
        };
        let mut tree = Tree::<CAPACITY, TEXT>::new();
        let root = build(&mut tree).unwrap();
        tree.set_root(root).unwrap();
        for &group in groups {
            tree.set_visible(GroupId::new(group), true).unwrap();
        }
        if let Some((switch, when)) = case {
            tree.set_case(SwitchId::new(switch), Some(when)).unwrap();
        }
        // A virtual list (contract v18) holds a thousand items, its rows
        // filled as slots are.
        if !slots.is_empty() {
            let lists: std::vec::Vec<nowayos_ui::ListId> = tree
                .scroll_views()
                .filter_map(|id| match tree.get(id).map(|node| node.kind()) {
                    Some(nowayos_ui::NodeKind::Scroll {
                        rows: Some(rows), ..
                    }) => Some(rows.list),
                    _ => None,
                })
                .collect();
            for list in lists {
                tree.set_list_count(list, 1_000).unwrap();
            }
        }
        for &slot in slots {
            // A long text (contract v16) shows a 10 KB article.
            let long = tree
                .find_slot(SlotId::new(slot))
                .and_then(|id| tree.get(id))
                .and_then(|node| tree.long_text_of(node))
                .is_some();
            if long {
                tree.set_slot(SlotId::new(slot), ARTICLE).unwrap();
                continue;
            }
            let text = std::format!("Text from the application, {slot}");
            match tree.set_slot(SlotId::new(slot), &text) {
                Err(UiError::NoGlyph) => {
                    let number = std::format!("A{slot:03}");
                    tree.set_slot(SlotId::new(slot), &number).unwrap();
                }
                // A progress bar, shown three fifths full.
                Err(UiError::UnknownSlot(_))
                    if tree.find_progress(SlotId::new(slot)).is_some() =>
                {
                    tree.set_progress(SlotId::new(slot), 3, 5).unwrap();
                }
                other => other.unwrap(),
            }
        }
        if errors {
            let fields: std::vec::Vec<nowayos_ui::FieldId> = tree
                .paint_order()
                .filter_map(|(_, node)| match node.kind() {
                    nowayos_ui::NodeKind::FieldError { field, .. } => Some(*field),
                    _ => None,
                })
                .collect();
            for field in fields {
                tree.set_field_error(field, "Error from the application")
                    .unwrap();
            }
        }
        tree.layout(Rect::new(0, 0, WIDTH, HEIGHT), &theme.metrics())
            .unwrap();
        // A text area (contract v19) holds what a person typed: a line
        // long enough to wrap, and a second paragraph.
        if errors {
            let areas: std::vec::Vec<nowayos_ui::FieldId> = tree
                .paint_order()
                .filter_map(|(_, node)| match node.kind() {
                    nowayos_ui::NodeKind::TextArea { field, .. } => Some(*field),
                    _ => None,
                })
                .collect();
            for field in areas {
                tree.set_value(
                    field,
                    "Text a person typed, long enough to wrap onto a second line at the width of the area.\nAnd a second paragraph.",
                )
                .unwrap();
            }
        }
        if scrolled {
            tree.scroll_to(u32::MAX).unwrap();
        }
        // One layer open over the screen (contract v20).
        if let Some(layer) = layer {
            tree.open(nowayos_ui::LayerId::new(layer)).unwrap();
        }
        if let Some(typing) = typing {
            let input = tree
                .paint_order()
                .find(|(_, node)| {
                    matches!(node.kind(), nowayos_ui::NodeKind::TextInput { mode, .. } if *mode == typing)
                })
                .map(|(id, _)| id);
            tree.set_focus(input).unwrap();
        }
        let mut frame = Framebuffer::new(WIDTH, HEIGHT).unwrap();
        nowayos_testkit::glyph_store!(glyphs);
        render(&tree, &mut frame, &theme, &mut glyphs).unwrap();
        frame
    }
"#;

/// Comparing a converted screen with its approved image, and checking that
/// no approved image is left without a screen: only when there are converted
/// screens.
const RUN_IMAGES: &str = r#"
    use std::format;
    use std::path::{Path, PathBuf};
    use std::string::String;
    use std::vec::Vec;

    use nowayos_testkit::check_snapshot;

    /// The approved images, one directory per fixture.
    fn golden() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("golden")
    }

    /// Draws a screen and compares the frame with its approved image, pixel
    /// for pixel - or, with NOWAYOS_UPDATE_SNAPSHOTS=1 (`make bless-images`),
    /// rewrites the image, to be reviewed by eye.
    fn matches_approved(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        image: &str,
    ) {
        if let Err(err) = check_snapshot(&frame(build), &golden().join(image)) {
            panic!("{err}\nIn screen-compiler, `make bless-images` redraws the approved images.");
        }
    }

    /// As `matches_approved_shown`, with the screen's scroll view scrolled
    /// to its end.
    fn matches_approved_scrolled(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        groups: &[u16],
        slots: &[u16],
        image: &str,
    ) {
        let frame = frame_in(false, build, groups, slots, None, true, None, true, None);
        if let Err(err) = check_snapshot(&frame, &golden().join(image)) {
            panic!("{err}\nIn screen-compiler, `make bless-images` redraws the approved images.");
        }
    }

    /// As `matches_approved`, with the first text input in `typing` focused
    /// so the on-screen keyboard shows its layout.
    fn matches_approved_keys(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        typing: nowayos_ui::InputMode,
        image: &str,
    ) {
        let frame = frame_in(false, build, &[], &[], None, false, Some(typing), false, None);
        if let Err(err) = check_snapshot(&frame, &golden().join(image)) {
            panic!("{err}\nIn screen-compiler, `make bless-images` redraws the approved images.");
        }
    }

    /// As `matches_approved`, for the frame in the large-text theme
    /// (`frame_large`).
    fn matches_approved_large(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        image: &str,
    ) {
        if let Err(err) = check_snapshot(&frame_large(build), &golden().join(image)) {
            panic!("{err}\nIn screen-compiler, `make bless-images` redraws the approved images.");
        }
    }

    /// As `matches_approved`, for the frame with every group visible and
    /// every slot filled (`frame_shown`).
    fn matches_approved_shown(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        groups: &[u16],
        slots: &[u16],
        image: &str,
    ) {
        let frame = frame_shown(build, groups, slots);
        if let Err(err) = check_snapshot(&frame, &golden().join(image)) {
            panic!("{err}\nIn screen-compiler, `make bless-images` redraws the approved images.");
        }
    }

    /// As `matches_approved_shown`, with `case`'s switch set to its case
    /// (`frame_case`).
    /// As `matches_approved_shown`, with layer `layer` open over the
    /// screen (contract v20).
    fn matches_approved_layer(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        groups: &[u16],
        slots: &[u16],
        layer: u16,
        image: &str,
    ) {
        let frame = frame_in(false, build, groups, slots, None, false, None, true, Some(layer));
        if let Err(err) = check_snapshot(&frame, &golden().join(image)) {
            panic!("{err}\nIn screen-compiler, `make bless-images` redraws the approved images.");
        }
    }

    fn matches_approved_case(
        build: impl FnOnce(&mut Tree<CAPACITY, TEXT>) -> Result<NodeId, UiError>,
        groups: &[u16],
        slots: &[u16],
        case: (u16, u8),
        image: &str,
    ) {
        let frame = frame_case(build, groups, slots, case);
        if let Err(err) = check_snapshot(&frame, &golden().join(image)) {
            panic!("{err}\nIn screen-compiler, `make bless-images` redraws the approved images.");
        }
    }

    /// Every file under golden/ is one of `images`: an image left behind by
    /// a removed fixture or a renamed screen fails here. Not counted: frames
    /// written beside a differing image (`*.actual.png`), hidden files (a
    /// file manager's `.DS_Store`), and a golden/ not yet written - the
    /// tests that write it may still be running (`make bless-images`).
    fn only_these_images(images: &[&str]) {
        let entries = match std::fs::read_dir(golden()) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) => panic!("golden/: {err}"),
        };
        let mut stray = Vec::new();
        for entry in entries {
            let entry = entry.unwrap();
            let name = String::from(entry.file_name().to_string_lossy());
            if name.starts_with('.') {
                continue;
            }
            if !entry.file_type().unwrap().is_dir() {
                stray.push(name);
                continue;
            }
            for file in std::fs::read_dir(entry.path()).unwrap() {
                let file = String::from(file.unwrap().file_name().to_string_lossy());
                let image = format!("{name}/{file}");
                if !file.starts_with('.')
                    && !file.ends_with(".actual.png")
                    && !images.contains(&image.as_str())
                {
                    stray.push(image);
                }
            }
        }
        stray.sort();
        assert!(
            stray.is_empty(),
            "golden/ holds files no converted screen draws - delete them: {stray:?}"
        );
    }
"#;

/// The fixture a converted file came from: its file stem, which must be
/// plain (letters, digits, `-` and `_`) because it names a directory of
/// approved images.
fn fixture_name(path: &Path) -> Result<String, Box<dyn Error>> {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    if stem.is_empty()
        || !stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(format!(
            "{}: a fixture's name may hold only letters, digits, - and _",
            path.display()
        )
        .into());
    }
    Ok(stem.to_owned())
}

/// Every `*.rs` file in `dir`, sorted, each in a module named
/// `PREFIX_STEM` (the stem with every other character made `_`).
fn generated(dir: &Path, prefix: &str) -> Result<Vec<Generated>, Box<dyn Error>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))? {
        let path = entry?.path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            paths.push(path);
        }
    }
    paths.sort();
    let mut files = Vec::new();
    for path in paths {
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        let name: String = stem
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect();
        let source = fs::read_to_string(&path)?;
        files.push(Generated {
            module: format!("{prefix}_{name}"),
            path,
            source,
            fixture: None,
        });
    }
    Ok(files)
}

/// The numbers of a screen's groups or slots: each `ui::TY::new(N)` inside
/// the top-level `pub mod SCREEN {` of a file of generated Rust, in order.
fn numbers(source: &str, screen: &str, ty: &str) -> Vec<u16> {
    let open = format!("pub mod {screen} {{");
    let call = format!("ui::{ty}::new(");
    let mut inside = false;
    let mut found = Vec::new();
    for line in source.lines() {
        if line == open {
            inside = true;
        } else if line == "}" {
            inside = false;
        } else if inside
            && let Some((_, rest)) = line.split_once(&call)
            && let Some((number, _)) = rest.split_once(')')
            && let Ok(number) = number.parse()
        {
            found.push(number);
        }
    }
    found
}

/// The layers of `screen` in a file of generated Rust (contract v20):
/// each one's number and name, lowercase with dashes.
fn layers(source: &str, screen: &str) -> Vec<(u16, String)> {
    let open = format!("pub mod {screen} {{");
    let mut inside = false;
    let mut found = Vec::new();
    for line in source.lines() {
        if line == open || line == "}" {
            inside = line == open;
            continue;
        }
        if inside
            && let Some((head, rest)) = line.trim().split_once(": ui::LayerId = ui::LayerId::new(")
            && let Some(name) = head.strip_prefix("pub const ")
            && let Some((number, _)) = rest.split_once(')')
            && let Ok(number) = number.parse()
        {
            found.push((number, name.to_lowercase().replace('_', "-")));
        }
    }
    found
}

/// A switch of a generated screen (contract v13).
struct Switch {
    /// Its number.
    number: u16,
    /// The name of its cases' module.
    name: String,
    /// Each case's number and name, lowercase.
    cases: Vec<(u8, String)>,
}

/// The switches of `screen`, in a file of generated Rust (contract v13):
/// each one's number and the name of its cases' module, and each case's
/// number and name, lowercase - from the shape the generator writes,
/// `pub const STEP: ui::SwitchId = ui::SwitchId::new(1);`, then
/// `pub mod step {` holding `pub const REVIEW: u8 = 0;`.
fn switches(source: &str, screen: &str) -> Vec<Switch> {
    let open = format!("pub mod {screen} {{");
    let mut inside = false;
    let mut found: Vec<Switch> = Vec::new();
    // The switch just numbered, until its cases' module opens; then in it.
    let mut numbered: Option<u16> = None;
    let mut in_cases = false;
    for line in source.lines() {
        if line == open || line == "}" {
            inside = line == open;
            continue;
        }
        if !inside {
            continue;
        }
        let text = line.trim();
        if let Some((_, rest)) = text.split_once("ui::SwitchId::new(")
            && let Some((number, _)) = rest.split_once(')')
            && let Ok(number) = number.parse()
        {
            numbered = Some(number);
        } else if let Some(name) = text
            .strip_prefix("pub mod ")
            .and_then(|rest| rest.strip_suffix(" {"))
            && let Some(number) = numbered.take()
        {
            found.push(Switch {
                number,
                name: name.to_owned(),
                cases: Vec::new(),
            });
            in_cases = true;
        } else if in_cases && text == "}" {
            in_cases = false;
        } else if in_cases
            && let Some(rest) = text.strip_prefix("pub const ")
            && let Some((name, rest)) = rest.split_once(": u8 = ")
            && let Some(when) = rest.strip_suffix(';')
            && let Ok(when) = when.parse()
            && let Some(switch) = found.last_mut()
        {
            switch.cases.push((when, name.to_ascii_lowercase()));
        }
    }
    found
}

/// Whether `screen`, in a file of generated Rust, names `what` - one of the
/// theme's spacing tokens, `ui::Space::` (contract v7), or colours,
/// `ui::Colour::` (contract v8).
fn screen_names(source: &str, screen: &str, what: &str) -> bool {
    let open = format!("pub mod {screen} {{");
    let mut inside = false;
    for line in source.lines() {
        if line == open {
            inside = true;
        } else if line == "}" {
            inside = false;
        } else if inside && line.contains(what) {
            return true;
        }
    }
    false
}

/// The modes `screen`'s text inputs take, in a file of generated Rust, each
/// once, in the order text, digits, email: its name in an image's and a
/// test's name, and its `nowayos_ui::InputMode` variant. An input
/// that names no mode takes text.
fn input_modes(source: &str, screen: &str) -> Vec<(&'static str, &'static str)> {
    let open = format!("pub mod {screen} {{");
    let mut inside = false;
    let mut seen = [false; 3];
    for line in source.lines() {
        if line == open {
            inside = true;
        } else if line == "}" {
            inside = false;
        } else if inside && line.contains(" = b.input(") {
            let index = if line.contains("ui::InputMode::Digits") {
                1
            } else if line.contains("ui::InputMode::Email") {
                2
            } else {
                0
            };
            if let Some(seen) = seen.get_mut(index) {
                *seen = true;
            }
        }
    }
    [("text", "Text"), ("digits", "Digits"), ("email", "Email")]
        .into_iter()
        .zip(seen)
        .filter_map(|(mode, seen)| seen.then_some(mode))
        .collect()
}

/// The screens in a file of generated Rust: each top-level `pub mod NAME {`
/// that holds a `pub fn build<B: ui::UiBuilder>(`.
fn screens(source: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current: Option<&str> = None;
    for line in source.lines() {
        if let Some(rest) = line.strip_prefix("pub mod ") {
            current = rest.strip_suffix(" {");
        } else if line.starts_with("    pub fn build<B: ui::UiBuilder>(")
            && let Some(name) = current.take()
        {
            found.push(name.to_owned());
        }
    }
    found
}

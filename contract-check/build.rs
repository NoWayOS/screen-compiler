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

#[allow(
    dead_code,
    reason = "shared with preview/build.rs, which uses the rest"
)]
#[path = "scan.rs"]
mod scan;

use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use scan::{Switch, input_modes, layers, numbers, screen_names, screens, switches};

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
    use nowayos_render_eg::Theme;
    use nowayos_testkit::Framebuffer;
    use nowayos_ui::{NodeId, Tree, UiError};
    use screen_compiler_preview::View;

    use super::{CAPACITY, TEXT};

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
    /// keyboard shows its layout; if `errors`, every field's
    /// error set to "Error from the application"; and given `layer`, that
    /// layer open (contract v20). The drawing itself is
    /// `screen_compiler_preview::frame`, which `make preview` uses too.
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
        let view = View {
            width: WIDTH,
            height: HEIGHT,
            large,
            groups: groups.to_vec(),
            slots: slots.iter().map(|&slot| (slot, None)).collect(),
            cases: case.into_iter().collect(),
            scrolled,
            typing,
            errors,
            layer,
        };
        screen_compiler_preview::frame(build, &view).unwrap()
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

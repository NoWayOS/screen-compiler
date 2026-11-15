// SPDX-License-Identifier: MIT
//! `make preview`'s program: draws each screen this build was given (the
//! Rust Screen Compiler made of one TSX file; see `build.rs`) and writes it
//! as a PNG.
//!
//! ```text
//! screen-compiler-preview OUT [--size WxH] [--large] [--show GROUP]...
//!     [--slot NAME=TEXT]... [--case SWITCH=CASE]... [--scrolled] [--errors]
//! ```
//!
//! With only a size or `--large`, each screen gives `OUT/SCREEN.png`, the
//! screen as it starts, and - if it has groups, slots or field errors -
//! `OUT/SCREEN.shown.png`, every group visible, every slot holding a sample
//! and every field's error set: the two pictures the contract check
//! approves for a fixture. Naming anything else gives one picture,
//! `OUT/SCREEN.png`, showing what was named. Names are the TSX's own - a
//! prop for a group or a slot, a switch's prop and a case's value - in any
//! case, with or without `-` and `_`.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use nowayos_ui::{NodeId, UiError};
use screen_compiler_preview::{Screen, View, frame};

/// The pictures generated code names at the crate's root (contract v15):
/// NoWayOS's samples.
#[allow(unused_imports, reason = "used only by a screen that shows a picture")]
mod pictures {
    pub use nowayos_ui::pictures::samples::*;
}

/// A screen this build can draw.
struct ScreenOf {
    /// Its module's name, which names its picture.
    name: &'static str,
    /// Builds it on a tree.
    build: fn(&mut Screen) -> Result<NodeId, UiError>,
    /// Its groups: the constant's name and the group's number.
    groups: &'static [(&'static str, u16)],
    /// Its slots, likewise.
    slots: &'static [(&'static str, u16)],
    /// Its switches.
    switches: &'static [SwitchOf],
    /// Whether it has a field's error to show.
    errors: bool,
}

/// A switch of a screen (contract v13).
struct SwitchOf {
    /// The name of its cases' module.
    name: &'static str,
    /// Its number.
    number: u16,
    /// Each case's number and name.
    cases: &'static [(u8, &'static str)],
}

include!(concat!(env!("OUT_DIR"), "/screens.rs"));

/// What the command line asked for.
#[derive(Default)]
struct Asked {
    /// The directory the pictures go in.
    out: Option<PathBuf>,
    /// The viewport, if not 800 x 480.
    size: Option<(u32, u32)>,
    /// The large-text, high-contrast theme.
    large: bool,
    /// Groups to show, as typed.
    show: Vec<String>,
    /// Slots to fill: name and text, as typed.
    slots: Vec<(String, String)>,
    /// Switches to set: switch and case, as typed.
    cases: Vec<(String, String)>,
    /// Scrolled to the end.
    scrolled: bool,
    /// Every field's error set.
    errors: bool,
}

impl Asked {
    /// Whether only the size and theme were chosen, so each screen gives its
    /// two usual pictures.
    fn plain(&self) -> bool {
        self.show.is_empty()
            && self.slots.is_empty()
            && self.cases.is_empty()
            && !self.scrolled
            && !self.errors
    }
}

const USAGE: &str = "usage: screen-compiler-preview OUT [--size WxH] [--large] [--show GROUP]... [--slot NAME=TEXT]... [--case SWITCH=CASE]... [--scrolled] [--errors]";

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("preview: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: impl Iterator<Item = String>) -> Result<(), String> {
    let asked = parse(args)?;
    let Some(out) = &asked.out else {
        return Err(USAGE.to_owned());
    };
    if SCREENS.is_empty() {
        return Err(
            "this build holds no screen: run it through `make preview FILE=screen.tsx`".to_owned(),
        );
    }
    let (width, height) = asked.size.unwrap_or((
        screen_compiler_preview::WIDTH,
        screen_compiler_preview::HEIGHT,
    ));
    let base = View {
        width,
        height,
        large: asked.large,
        ..View::default()
    };
    // Each name typed, and whether any screen had it; and every picture to
    // draw, none drawn until every name is known.
    let mut used = vec![false; names(&asked).len()];
    let mut pictures: Vec<(&ScreenOf, &str, View)> = Vec::new();
    for screen in SCREENS {
        if asked.plain() {
            pictures.push((screen, "", base.clone()));
            if !screen.groups.is_empty() || !screen.slots.is_empty() || screen.errors {
                let shown = View {
                    groups: screen.groups.iter().map(|&(_, number)| number).collect(),
                    slots: screen
                        .slots
                        .iter()
                        .map(|&(_, number)| (number, None))
                        .collect(),
                    errors: true,
                    ..base.clone()
                };
                pictures.push((screen, ".shown", shown));
            }
            continue;
        }
        let mut view = View {
            scrolled: asked.scrolled,
            errors: asked.errors,
            ..base.clone()
        };
        let mut seen = used.iter_mut();
        for name in &asked.show {
            let found = find(screen.groups, name);
            view.groups.extend(found);
            mark(seen.next(), found.is_some());
        }
        for (name, text) in &asked.slots {
            let found = find(screen.slots, name);
            view.slots
                .extend(found.map(|number| (number, Some(text.clone()))));
            mark(seen.next(), found.is_some());
        }
        for (name, case) in &asked.cases {
            let found = screen
                .switches
                .iter()
                .find(|switch| same(switch.name, name))
                .and_then(|switch| {
                    let (when, _) = switch.cases.iter().find(|(_, known)| same(known, case))?;
                    Some((switch.number, *when))
                });
            view.cases.extend(found);
            mark(seen.next(), found.is_some());
        }
        pictures.push((screen, "", view));
    }
    let unknown: Vec<String> = names(&asked)
        .into_iter()
        .zip(used)
        .filter_map(|(name, used)| (!used).then_some(name))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "no screen has {}; there are: {}",
            unknown.join(", "),
            known()
        ));
    }
    fs::create_dir_all(out).map_err(|err| format!("{}: {err}", out.display()))?;
    for (screen, kind, view) in pictures {
        write(out, screen.name, kind, &draw(screen, &view)?)?;
    }
    Ok(())
}

/// Marks a typed name as found in some screen.
fn mark(slot: Option<&mut bool>, found: bool) {
    if let Some(slot) = slot
        && found
    {
        *slot = true;
    }
}

/// Every name typed, in the order `run` looks them up: groups, slots, then
/// switches with their cases.
fn names(asked: &Asked) -> Vec<String> {
    let groups = asked.show.iter().map(|name| format!("group {name}"));
    let slots = asked.slots.iter().map(|(name, _)| format!("slot {name}"));
    let cases = asked
        .cases
        .iter()
        .map(|(name, case)| format!("switch {name} with case {case}"));
    groups.chain(slots).chain(cases).collect()
}

/// What this build's screens have, for the message when a name is unknown.
fn known() -> String {
    let mut found = Vec::new();
    for screen in SCREENS {
        for (name, _) in screen.groups {
            found.push(format!("group {name}"));
        }
        for (name, _) in screen.slots {
            found.push(format!("slot {name}"));
        }
        for switch in screen.switches {
            let cases: Vec<&str> = switch.cases.iter().map(|&(_, case)| case).collect();
            found.push(format!("switch {} ({})", switch.name, cases.join(", ")));
        }
    }
    if found.is_empty() {
        "no groups, slots or switches".to_owned()
    } else {
        found.join(", ")
    }
}

/// The number that goes with `name` among a screen's groups or slots.
fn find(list: &[(&str, u16)], name: &str) -> Option<u16> {
    list.iter()
        .find(|(known, _)| same(known, name))
        .map(|&(_, number)| number)
}

/// Whether two names are one: letters and digits compared without case,
/// everything else ignored, so `phoneError`, `phone-error` and `PHONE_ERROR`
/// agree.
fn same(a: &str, b: &str) -> bool {
    let plain = |text: &str| -> String {
        text.chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect()
    };
    plain(a) == plain(b)
}

/// Draws one screen as a PNG.
fn draw(screen: &ScreenOf, view: &View) -> Result<Vec<u8>, String> {
    frame(screen.build, view)
        .map_err(|err| format!("{}: {err}", screen.name))?
        .to_png()
        .map_err(|err| format!("{}: {err}", screen.name))
}

/// Writes `OUT/SCREEN[KIND].png` whole - beside it first, then moved into
/// place, so a viewer never reads half a picture - and says so.
fn write(out: &Path, screen: &str, kind: &str, png: &[u8]) -> Result<(), String> {
    let path = out.join(format!("{screen}{kind}.png"));
    let partial = out.join(format!(".{screen}{kind}.png.partial"));
    fs::write(&partial, png).map_err(|err| format!("{}: {err}", partial.display()))?;
    fs::rename(&partial, &path).map_err(|err| format!("{}: {err}", path.display()))?;
    println!("{}", path.display());
    Ok(())
}

/// Reads the command line.
fn parse(mut args: impl Iterator<Item = String>) -> Result<Asked, String> {
    let mut asked = Asked::default();
    while let Some(arg) = args.next() {
        let mut value = |what: &str| {
            args.next()
                .ok_or_else(|| format!("{arg} needs {what}\n{USAGE}"))
        };
        match arg.as_str() {
            "--size" => {
                let text = value("a size, WIDTHxHEIGHT")?;
                let size = text
                    .split_once(['x', 'X'])
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .filter(|&(w, h)| w > 0 && h > 0);
                let Some(size) = size else {
                    return Err(format!(
                        "--size takes WIDTHxHEIGHT, such as 1920x1080, not {text:?}"
                    ));
                };
                asked.size = Some(size);
            }
            "--large" => asked.large = true,
            "--scrolled" => asked.scrolled = true,
            "--errors" => asked.errors = true,
            "--show" => asked.show.push(value("a group's name")?),
            "--slot" => {
                let text = value("NAME=TEXT")?;
                let Some((name, text)) = text.split_once('=') else {
                    return Err(format!("--slot takes NAME=TEXT, not {text:?}"));
                };
                asked.slots.push((name.to_owned(), text.to_owned()));
            }
            "--case" => {
                let text = value("SWITCH=CASE")?;
                let Some((name, case)) = text.split_once('=') else {
                    return Err(format!("--case takes SWITCH=CASE, not {text:?}"));
                };
                asked.cases.push((name.to_owned(), case.to_owned()));
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown option {other}\n{USAGE}"));
            }
            _ if asked.out.is_none() => asked.out = Some(PathBuf::from(&arg)),
            _ => return Err(format!("one output directory only\n{USAGE}")),
        }
    }
    Ok(asked)
}

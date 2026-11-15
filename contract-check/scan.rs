// SPDX-License-Identifier: MIT
//! Reading the fixed shape of the Rust Screen Compiler generates: a file's
//! screens, and each screen's groups, slots, switches, layers and input
//! modes. Shared by the two build scripts, `build.rs` here and
//! `preview/build.rs`, each of which uses part of it.

/// The names and numbers of a screen's groups or slots: each
/// `pub const NAME: ui::TY = ui::TY::new(N);` inside the top-level
/// `pub mod SCREEN {` of a file of generated Rust, in order.
pub(crate) fn named(source: &str, screen: &str, ty: &str) -> Vec<(String, u16)> {
    let open = format!("pub mod {screen} {{");
    let call = format!(": ui::{ty} = ui::{ty}::new(");
    let mut inside = false;
    let mut found = Vec::new();
    for line in source.lines() {
        if line == open || line == "}" {
            inside = line == open;
        } else if inside
            && let Some((head, rest)) = line.trim().split_once(&call)
            && let Some(name) = head.strip_prefix("pub const ")
            && let Some((number, _)) = rest.split_once(')')
            && let Ok(number) = number.parse()
        {
            found.push((name.to_owned(), number));
        }
    }
    found
}

/// The numbers of a screen's groups or slots: each `ui::TY::new(N)` inside
/// the top-level `pub mod SCREEN {` of a file of generated Rust, in order.
pub(crate) fn numbers(source: &str, screen: &str, ty: &str) -> Vec<u16> {
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
pub(crate) fn layers(source: &str, screen: &str) -> Vec<(u16, String)> {
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
pub(crate) struct Switch {
    /// Its number.
    pub(crate) number: u16,
    /// The name of its cases' module.
    pub(crate) name: String,
    /// Each case's number and name, lowercase.
    pub(crate) cases: Vec<(u8, String)>,
}

/// The switches of `screen`, in a file of generated Rust (contract v13):
/// each one's number and the name of its cases' module, and each case's
/// number and name, lowercase - from the shape the generator writes,
/// `pub const STEP: ui::SwitchId = ui::SwitchId::new(1);`, then
/// `pub mod step {` holding `pub const REVIEW: u8 = 0;`.
pub(crate) fn switches(source: &str, screen: &str) -> Vec<Switch> {
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
pub(crate) fn screen_names(source: &str, screen: &str, what: &str) -> bool {
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
pub(crate) fn input_modes(source: &str, screen: &str) -> Vec<(&'static str, &'static str)> {
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
pub(crate) fn screens(source: &str) -> Vec<String> {
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

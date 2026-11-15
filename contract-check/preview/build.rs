// SPDX-License-Identifier: MIT
//! Writes `screens.rs` into `OUT_DIR`: one module per file of generated Rust
//! in the directory `SCREEN_COMPILER_PREVIEW` names (`make preview` converts
//! one TSX file into it), holding the file exactly as Screen Compiler wrote
//! it, and a table of its screens - each one's name, a function that builds
//! it on a real nowayos-ui tree, and the names and numbers of its groups,
//! slots and switches, so the program can show them by name. With no
//! directory named the table is empty.
//!
//! Finding the screens reads the generated code's fixed shape, as the
//! contract check's build script does (`../scan.rs`).

#[allow(dead_code, reason = "shared with ../build.rs, which uses the rest")]
#[path = "../scan.rs"]
mod scan;

use std::collections::HashSet;
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use scan::{named, screen_names, screens, switches};

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-env-changed=SCREEN_COMPILER_PREVIEW");
    let mut out = String::new();
    let mut table = String::new();
    if let Some(dir) = env::var_os("SCREEN_COMPILER_PREVIEW") {
        let dir = PathBuf::from(dir);
        println!("cargo::rerun-if-changed={}", dir.display());
        let mut paths = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|err| format!("{}: {err}", dir.display()))? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == "rs") {
                paths.push(path);
            }
        }
        paths.sort();
        let mut modules: HashSet<String> = HashSet::new();
        for path in paths {
            let shown = path.display().to_string();
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
            let module = format!("generated_{name}");
            if !modules.insert(module.clone()) {
                return Err(format!(
                    "{shown}: its module name, {module}, is taken by another file: rename one"
                )
                .into());
            }
            let source = fs::read_to_string(&path)?;
            let found = screens(&source);
            if found.is_empty() {
                return Err(format!("{shown}: no screen found (no `pub fn build`)").into());
            }
            writeln!(out, "/// `{shown}`, as Screen Compiler wrote it.")?;
            writeln!(
                out,
                "#[allow(dead_code, reason = \"a preview only builds each screen: its actions, fields and handles go unused\")]"
            )?;
            writeln!(out, "mod {module} {{")?;
            writeln!(out, "    include!({shown:?});")?;
            writeln!(out, "}}")?;
            for screen in found {
                writeln!(out, "/// `{module}::{screen}` on a real tree.")?;
                writeln!(
                    out,
                    "fn build_{module}_{screen}(tree: &mut Screen) -> Result<NodeId, UiError> {{"
                )?;
                writeln!(
                    out,
                    "    {module}::{screen}::build(tree).map(|nodes| nodes.root)"
                )?;
                writeln!(out, "}}")?;
                let groups = named(&source, &screen, "GroupId");
                let slots = named(&source, &screen, "SlotId");
                let errors = screen_names(&source, &screen, "b.field_error(");
                writeln!(table, "    ScreenOf {{")?;
                writeln!(table, "        name: {screen:?},")?;
                writeln!(table, "        build: build_{module}_{screen},")?;
                writeln!(table, "        groups: &{groups:?},")?;
                writeln!(table, "        slots: &{slots:?},")?;
                writeln!(table, "        switches: &[")?;
                for switch in switches(&source, &screen) {
                    writeln!(
                        table,
                        "            SwitchOf {{ name: {:?}, number: {}, cases: &{:?} }},",
                        switch.name, switch.number, switch.cases
                    )?;
                }
                writeln!(table, "        ],")?;
                writeln!(table, "        errors: {errors},")?;
                writeln!(table, "    }},")?;
            }
        }
    }
    writeln!(
        out,
        "/// Every screen in the files converted for this build."
    )?;
    writeln!(out, "static SCREENS: &[ScreenOf] = &[")?;
    out.push_str(&table);
    writeln!(out, "];")?;
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    fs::write(out_dir.join("screens.rs"), out)?;
    Ok(())
}

// SPDX-License-Identifier: MIT
//! The coverage report: how much of a folder of React code Screen Compiler
//! converts (`screen-compiler audit DIR`).
//!
//! Each file is converted, as `check` would, and the census of what it
//! writes ([`screen_compiler_tsx::census()`]) is judged construct by
//! construct ([`verdict`]): in the subset, a step on the road towards
//! React and CSS, out by design, or unknown. A construct this table does
//! not know is unknown, never counted as converting; and a file converts
//! only if the analyser converts it, whatever its constructs are judged.
//! The report names constructs and counts them; it quotes no code.
//!
//! Run it only on code you trust: the parser's limits are those of
//! `check` and `convert`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use screen_compiler_ir::escape_special;
use screen_compiler_tsx::census::{Census, Construct, ImportKind};

use crate::kit::KitComponent;
use crate::{Options, analyse};

/// The most files one report reads.
pub const MAX_FILES: usize = 10_000;

/// A step on the road towards React and CSS: what would convert a gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Step {
    /// Fragments and the components a screen's authors write.
    Components,
    /// Text mixing literals and props.
    MixedText,
    /// Lists from `.map`.
    Lists,
    /// More of CSS layout: flex growth, margins, per-side padding.
    CssLayout,
    /// Conditions beyond one prop: negation, comparison with a literal.
    Conditions,
    /// React DOM's event names, and arrows that only call a prop.
    DomEvents,
    /// Common HTML elements, by fixed rules.
    HtmlElements,
    /// `className` with CSS modules.
    CssModules,
    /// Elements laid over one another.
    Layering,
    /// Images and masked entry.
    ImagesAndMasking,
}

impl Step {
    /// The step, as the report names it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Components => "fragments and your own components",
            Self::MixedText => "text mixing literals and props",
            Self::Lists => "lists",
            Self::CssLayout => "more of CSS layout",
            Self::Conditions => "more conditions",
            Self::DomEvents => "React DOM's event names",
            Self::HtmlElements => "common HTML elements",
            Self::CssModules => "CSS modules",
            Self::Layering => "layering",
            Self::ImagesAndMasking => "images and masked entry",
        }
    }
}

/// What the subset makes of a construct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// The subset has it (where it is used as the subset allows).
    Subset,
    /// A step on the road converts it.
    Road(Step),
    /// Out by design, and why.
    Out(&'static str),
    /// Not known to the subset or to any step on the road.
    Unknown,
}

const STATE: &str = "state and hooks live in the application, which sets what a screen shows";
const FIELDS: &str = "a field is read and set by the application through its id";
const SPREAD: &str = "a spread's props are known only when the code runs";
const COMPUTED_STYLE: &str = "a style computed when the code runs";
const CASCADE: &str = "a global stylesheet: there is no cascade";
const ANIMATION: &str = "animation";

/// The style properties the road's CSS layout step adds.
const LAYOUT: &[&str] = &[
    "flex",
    "flexGrow",
    "flexShrink",
    "flexBasis",
    "flexDirection",
    "alignSelf",
    "minHeight",
    "maxHeight",
    "margin",
    "marginTop",
    "marginRight",
    "marginBottom",
    "marginLeft",
    "marginHorizontal",
    "marginVertical",
    "paddingTop",
    "paddingRight",
    "paddingBottom",
    "paddingLeft",
    "paddingHorizontal",
    "paddingVertical",
    "display",
];

/// The style properties the road's layering step adds.
const POSITION: &[&str] = &["position", "top", "right", "bottom", "left"];

/// The HTML elements the road maps by fixed rules.
const HTML: &[&str] = &[
    "div", "span", "p", "h1", "h2", "h3", "h4", "h5", "h6", "label", "button", "input", "hr",
];

/// What the subset makes of `construct`.
#[must_use]
pub fn verdict(construct: &Construct) -> Verdict {
    match construct {
        Construct::Component(name) => component(name),
        Construct::Html(name) => match name.as_str() {
            "img" => Verdict::Road(Step::ImagesAndMasking),
            name if HTML.contains(&name) => Verdict::Road(Step::HtmlElements),
            _ => Verdict::Unknown,
        },
        Construct::OtherElement => Verdict::Unknown,
        Construct::Fragment => Verdict::Road(Step::Components),
        Construct::Prop { element, prop } => prop_verdict(element, prop),
        Construct::SpreadProp => Verdict::Out(SPREAD),
        Construct::Style { element, property } => style_verdict(element, property),
        Construct::ComputedStyle => Verdict::Out(COMPUTED_STYLE),
        Construct::MixedText | Construct::Template => Verdict::Road(Step::MixedText),
        Construct::Hook(_) | Construct::Class => Verdict::Out(STATE),
        Construct::Map => Verdict::Road(Step::Lists),
        Construct::Call(_) => Verdict::Unknown,
        Construct::Binary(op) => match *op {
            "===" | "!==" | "==" | "!=" | "<" | ">" | "<=" | ">=" => {
                Verdict::Road(Step::Conditions)
            }
            _ => Verdict::Unknown,
        },
        Construct::Unary(op) => match *op {
            "!" => Verdict::Road(Step::Conditions),
            _ => Verdict::Unknown,
        },
        Construct::Logical(op) => match *op {
            "&&" => Verdict::Subset,
            _ => Verdict::Unknown,
        },
        Construct::Conditional => Verdict::Subset,
        Construct::InlineFunction { .. } => Verdict::Road(Step::DomEvents),
        Construct::Import(kind) => match kind {
            ImportKind::Kit | ImportKind::React => Verdict::Subset,
            ImportKind::CssModule => Verdict::Road(Step::CssModules),
            ImportKind::Stylesheet => Verdict::Out(CASCADE),
            ImportKind::Local => Verdict::Road(Step::Components),
            ImportKind::Package(_) => Verdict::Unknown,
        },
        Construct::Statement(kind) => match *kind {
            "if" | "switch" => Verdict::Road(Step::Conditions),
            _ => Verdict::Unknown,
        },
    }
}

/// A capitalised element: the kit's, or one the road brings.
fn component(name: &str) -> Verdict {
    match name {
        _ if KitComponent::from_name(name).is_some() => Verdict::Subset,
        "Image" => Verdict::Road(Step::ImagesAndMasking),
        "Stack" => Verdict::Road(Step::Layering),
        _ => Verdict::Road(Step::Components),
    }
}

/// A prop on `element`.
fn prop_verdict(element: &str, prop: &str) -> Verdict {
    let road = |step| Verdict::Road(step);
    match (KitComponent::from_name(element), prop) {
        (_, "onClick") => road(Step::DomEvents),
        (_, "className" | "class") => road(Step::CssModules),
        (_, "key") => road(Step::Lists),
        (Some(kit), prop) if kit.prop(prop).is_some() => Verdict::Subset,
        (Some(KitComponent::TextInput), "type") => road(Step::ImagesAndMasking),
        (
            Some(KitComponent::TextInput | KitComponent::Checkbox | KitComponent::RadioGroup),
            "value" | "defaultValue" | "checked" | "defaultChecked" | "onChange",
        ) => Verdict::Out(FIELDS),
        (Some(_), _) => Verdict::Unknown,
        // A prop of an HTML element, or of a component the road brings, comes
        // with its element.
        (None, _) => match verdict(&element_construct(element)) {
            Verdict::Road(step) => road(step),
            _ => Verdict::Unknown,
        },
    }
}

/// A property of a `style` on `element`.
fn style_verdict(element: &str, property: &str) -> Verdict {
    let declared = |property: &str| KitComponent::ALL.iter().any(|kit| kit.styles(property));
    match KitComponent::from_name(element) {
        Some(kit) if kit.styles(property) => Verdict::Subset,
        _ if LAYOUT.contains(&property) => Verdict::Road(Step::CssLayout),
        _ if POSITION.contains(&property) || property == "zIndex" => Verdict::Road(Step::Layering),
        _ if property.starts_with("animation") || property.starts_with("transition") => {
            Verdict::Out(ANIMATION)
        }
        // A declared property on an element the road brings comes with it.
        None if declared(property) => match verdict(&element_construct(element)) {
            Verdict::Road(step) => Verdict::Road(step),
            _ => Verdict::Unknown,
        },
        _ => Verdict::Unknown,
    }
}

/// The element construct for a name as written.
fn element_construct(element: &str) -> Construct {
    if element.chars().next().is_some_and(char::is_lowercase) {
        Construct::Html(element.to_owned())
    } else if element.is_empty() {
        Construct::OtherElement
    } else {
        Construct::Component(element.to_owned())
    }
}

/// A construct as the report names it.
fn describe(construct: &Construct) -> String {
    match construct {
        Construct::Component(name) => format!("<{name}>"),
        Construct::Html(name) => format!("<{name}>"),
        Construct::OtherElement => "a member or namespaced element".to_owned(),
        Construct::Fragment => "<>".to_owned(),
        Construct::Prop { element, prop } => format!("{prop} on <{element}>"),
        Construct::SpreadProp => "{...props}".to_owned(),
        Construct::Style { element, property } => format!("style {property} on <{element}>"),
        Construct::ComputedStyle => "a computed style".to_owned(),
        Construct::MixedText => "text mixed with {…}".to_owned(),
        Construct::Hook(name) => format!("{name}()"),
        Construct::Map => ".map()".to_owned(),
        Construct::Call(name) if name.is_empty() => "a call".to_owned(),
        Construct::Call(name) => format!("{name}()"),
        Construct::Template => "a template literal".to_owned(),
        Construct::Binary(op) | Construct::Unary(op) | Construct::Logical(op) => (*op).to_owned(),
        Construct::Conditional => "?:".to_owned(),
        Construct::InlineFunction { element, prop } => {
            format!("an inline function as {prop} on <{element}>")
        }
        Construct::Import(kind) => match kind {
            ImportKind::Kit => "an import from the kit".to_owned(),
            ImportKind::React => "an import from react".to_owned(),
            ImportKind::CssModule => "an import of a CSS module".to_owned(),
            ImportKind::Stylesheet => "an import of a stylesheet".to_owned(),
            ImportKind::Local => "an import of a local file".to_owned(),
            ImportKind::Package(name) => format!("an import from {name}"),
        },
        Construct::Class => "a class".to_owned(),
        Construct::Statement(kind) => format!("a statement: {kind}"),
    }
}

/// What became of one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileResult {
    /// It parsed: whether it converts, and what it writes.
    Read {
        /// Whether the analyser converts it.
        converts: bool,
        /// What it writes.
        census: Census,
    },
    /// It could not be read as TSX: not UTF-8, too large, too deeply
    /// nested, or not parsing.
    Unreadable,
}

/// One file's result.
#[must_use]
pub fn audit_source(source: &str, name: &str) -> FileResult {
    let Ok(census) = screen_compiler_tsx::census(source) else {
        return FileResult::Unreadable;
    };
    let converts = screen_compiler_tsx::parse(source).is_ok_and(|module| {
        let analysis = analyse(&module, &Options::new(name));
        analysis.module.is_some()
    });
    FileResult::Read { converts, census }
}

/// Why a folder could not be audited.
#[derive(Debug)]
pub enum AuditError {
    /// A folder or file could not be read.
    Io(PathBuf, io::Error),
    /// More than [`MAX_FILES`] `.tsx` files.
    TooManyFiles,
}

/// Every `.tsx` file under `dir`, sorted; symbolic links are not followed.
///
/// # Errors
///
/// [`AuditError`] if a folder cannot be read or there are too many files.
fn tsx_files(dir: &Path) -> Result<Vec<PathBuf>, AuditError> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(folder) = pending.pop() {
        let entries =
            fs::read_dir(&folder).map_err(|error| AuditError::Io(folder.clone(), error))?;
        for entry in entries {
            let entry = entry.map_err(|error| AuditError::Io(folder.clone(), error))?;
            let kind = entry
                .file_type()
                .map_err(|error| AuditError::Io(entry.path(), error))?;
            let path = entry.path();
            if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "tsx") {
                if found.len() >= MAX_FILES {
                    return Err(AuditError::TooManyFiles);
                }
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// The report on `dir`: every `.tsx` file under it.
///
/// # Errors
///
/// [`AuditError`] if a folder cannot be read or there are too many files. A
/// file that cannot be read as TSX is counted, not an error.
pub fn audit_dir(dir: &Path) -> Result<Report, AuditError> {
    let mut report = Report::default();
    for path in tsx_files(dir)? {
        let shown = path
            .strip_prefix(dir)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let result = match fs::read(&path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(source) => audit_source(&source, &shown),
                Err(_) => FileResult::Unreadable,
            },
            Err(error) => return Err(AuditError::Io(path, error)),
        };
        report.files.push((shown, result));
    }
    Ok(report)
}

/// The files of one report, in path order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Each file's path below the folder, and its result.
    pub files: Vec<(String, FileResult)>,
}

/// Counts by verdict.
#[derive(Default)]
struct Tally {
    by_verdict: BTreeMap<Verdict, u64>,
    total: u64,
}

impl Tally {
    fn add(&mut self, verdict: Verdict, count: u32) {
        let slot = self.by_verdict.entry(verdict).or_insert(0);
        *slot = slot.saturating_add(u64::from(count));
        self.total = self.total.saturating_add(u64::from(count));
    }

    /// How many are in the subset, on the road, out by design and unknown.
    fn kinds(&self) -> [u64; 4] {
        let mut kinds = [0u64; 4];
        for (verdict, count) in &self.by_verdict {
            let index = match verdict {
                Verdict::Subset => 0,
                Verdict::Road(_) => 1,
                Verdict::Out(_) => 2,
                Verdict::Unknown => 3,
            };
            if let Some(slot) = kinds.get_mut(index) {
                *slot = slot.saturating_add(*count);
            }
        }
        kinds
    }
}

/// `part` of `whole` as a percentage, to one decimal place.
fn percent(part: u64, whole: u64) -> String {
    let tenths = part.saturating_mul(1000).checked_div(whole).unwrap_or(0);
    format!("{}.{}%", tenths / 10, tenths % 10)
}

impl Report {
    /// The report as text, the same for the same files every time.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        let mut converts = 0u64;
        let mut not = 0u64;
        let mut unreadable = 0u64;
        let mut tally = Tally::default();
        let mut road: BTreeMap<Step, (u64, u64)> = BTreeMap::new();
        let mut out_of: BTreeMap<&'static str, BTreeMap<String, u64>> = BTreeMap::new();
        let mut unknown: BTreeMap<String, u64> = BTreeMap::new();
        for (_, result) in &self.files {
            let FileResult::Read {
                converts: c,
                census,
            } = result
            else {
                unreadable = unreadable.saturating_add(1);
                continue;
            };
            if *c {
                converts = converts.saturating_add(1);
            } else {
                not = not.saturating_add(1);
            }
            let mut steps_here = BTreeMap::new();
            for (construct, count) in &census.counts {
                let verdict = verdict(construct);
                tally.add(verdict, *count);
                match verdict {
                    Verdict::Subset => {}
                    Verdict::Road(step) => {
                        let entry = steps_here.entry(step).or_insert(0u64);
                        *entry = entry.saturating_add(u64::from(*count));
                    }
                    Verdict::Out(why) => {
                        let entry = out_of
                            .entry(why)
                            .or_default()
                            .entry(describe(construct))
                            .or_insert(0);
                        *entry = entry.saturating_add(u64::from(*count));
                    }
                    Verdict::Unknown => {
                        let entry = unknown.entry(describe(construct)).or_insert(0);
                        *entry = entry.saturating_add(u64::from(*count));
                    }
                }
            }
            for (step, count) in steps_here {
                let entry = road.entry(step).or_insert((0, 0));
                entry.0 = entry.0.saturating_add(count);
                entry.1 = entry.1.saturating_add(1);
            }
        }
        let files = converts.saturating_add(not).saturating_add(unreadable);
        let _ = writeln!(out, "screen-compiler audit: {files} files");
        let _ = writeln!(
            out,
            "  convert            {converts:>6}  {}",
            percent(converts, files)
        );
        let _ = writeln!(out, "  do not convert     {not:>6}");
        let _ = writeln!(out, "  cannot be read     {unreadable:>6}");
        let [subset, on_road, out_by_design, not_known] = tally.kinds();
        let _ = writeln!(out, "\nconstructs         {:>6}", tally.total);
        for (label, count) in [
            ("in the subset", subset),
            ("on the road", on_road),
            ("out by design", out_by_design),
            ("unknown", not_known),
        ] {
            let _ = writeln!(
                out,
                "  {label:<17}{count:>6}  {}",
                percent(count, tally.total)
            );
        }
        if !road.is_empty() {
            let _ = writeln!(out, "\non the road");
            for (step, (count, in_files)) in &road {
                let _ = writeln!(
                    out,
                    "  {:<36}{count:>6} in {in_files} file{}",
                    step.name(),
                    if *in_files == 1 { "" } else { "s" }
                );
            }
        }
        if !out_of.is_empty() {
            let _ = writeln!(out, "\nout by design");
            for (why, constructs) in &out_of {
                let _ = writeln!(out, "  {why}");
                for (construct, count) in constructs {
                    let _ = writeln!(out, "    {:<34}{count:>6}", escape_special(construct));
                }
            }
        }
        if !unknown.is_empty() {
            let _ = writeln!(out, "\nunknown");
            for (construct, count) in &unknown {
                let _ = writeln!(out, "  {:<36}{count:>6}", escape_special(construct));
            }
        }
        let _ = writeln!(out, "\nfiles");
        for (path, result) in &self.files {
            let what = match result {
                FileResult::Unreadable => "cannot be read".to_owned(),
                FileResult::Read { converts: true, .. } => "converts".to_owned(),
                FileResult::Read { census, .. } => {
                    let mut file = Tally::default();
                    for (construct, count) in &census.counts {
                        file.add(verdict(construct), *count);
                    }
                    let [_, on_road, out_by_design, not_known] = file.kinds();
                    format!(
                        "does not convert: {on_road} on the road, {out_by_design} out by design, {not_known} unknown"
                    )
                }
            };
            let _ = writeln!(out, "  {}  {what}", escape_special(path));
        }
        out
    }
}

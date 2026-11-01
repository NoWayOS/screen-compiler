// SPDX-License-Identifier: MIT
//! Subset checking for Screen Compiler: TSX syntax tree to UI IR.
//!
//! [`analyse`] walks the syntax tree from `screen-compiler-tsx`, checks every
//! construct against **the subset** (v0, and the parts of v1 built so far),
//! and lowers what it accepts into the IR
//! (`screen-compiler-ir`). Checking and lowering are one walk, so the IR can only
//! contain what was checked. Anything outside the subset is reported with its
//! line and column and is **never approximated**: if there is any error, no IR
//! is produced.
//!
//! Subset v0:
//!
//! * top level: named imports from the kit module ([`KIT_MODULE`]), `import
//!   React from "react"`, type-only imports and declarations, integer design
//!   tokens (`const SPACE_MD = 16;`) and screen components;
//! * a screen component is a named function (or `const` arrow function) whose
//!   body is only `return <Screen>…</Screen>`; its parameter is either
//!   destructured props or a single props object;
//! * elements: `Screen` (exactly one child), `Column` and `Row` (`gap`,
//!   `padding`), `Text` (literal text), `Button` (literal label, `onPress`
//!   naming a callback prop), `TextInput` (`placeholder`, required `id`) and
//!   `Spacer` (optional `size`); every element accepts `id`;
//! * values: string literals, whole numbers of pixels, design tokens;
//! * subset v1 so far: a `Text` whose only content is one prop
//!   (`<Text>{props.name}</Text>`) becomes a slot the application fills; and
//!   `{props.NAME && <X/>}` and `{props.NAME ? <X/> : <Y/>}` (or `: null`)
//!   become a group the application shows or hides, each named after its
//!   prop. A condition is one prop, and what it shows one element.
//!

#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

mod kit;
mod lower;

pub use kit::KitComponent;
use screen_compiler_ir::{Diagnostic, Module};
use screen_compiler_tsx::SourceModule;

/// The module screens import the component kit from.
///
/// A virtual specifier: no package manager can resolve it, so the kit can
/// never be confused with a published package of the same name.
pub const KIT_MODULE: &str = "nowayos:kit";

/// Bounds on what one screen may contain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Deepest element nesting inside `<Screen>` (its child is depth 1).
    /// The default, 16, keeps generated screens within the first-cut
    /// nowayos-ui tree (`MAX_DEPTH` 16, counted from its root at 0).
    pub max_depth: usize,
    /// Most elements in one screen.
    pub max_nodes: usize,
    /// Longest text, label or placeholder, in bytes of UTF-8.
    pub max_text_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 16,
            max_nodes: 512,
            max_text_bytes: 1024,
        }
    }
}

/// How to analyse one source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// The source file's name, recorded in the IR (not a full path).
    pub source_name: String,
    /// The module specifier the kit is imported from.
    pub kit_module: String,
    /// Size bounds.
    pub limits: Limits,
}

impl Options {
    /// Default options for a file called `source_name`.
    #[must_use]
    pub fn new(source_name: impl Into<String>) -> Self {
        Self {
            source_name: source_name.into(),
            kit_module: KIT_MODULE.to_owned(),
            limits: Limits::default(),
        }
    }
}

/// The result of analysing one file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Analysis {
    /// The IR, present only when there are no errors.
    pub module: Option<Module>,
    /// Every finding, sorted by position.
    pub diagnostics: Vec<Diagnostic>,
}

/// Checks `source` against the subset and lowers it to the IR.
#[must_use]
pub fn analyse(source: &SourceModule, options: &Options) -> Analysis {
    lower::analyse(source, options)
}

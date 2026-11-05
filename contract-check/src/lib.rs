// SPDX-License-Identifier: MIT
//! Every screen Screen Compiler generates, built against the real `nowayos-ui`.
//!
//! `build.rs` puts each file of generated Rust in a module of its own,
//! exactly as Screen Compiler wrote it: every checked-in snapshot (`snapshot_*`),
//! and every fixture `make contract-check` converted afresh (`converted_*`).
//! Generated code imports `nowayos_ui::builder`; here that is NoWayOS's real
//! UI core, from the sibling clone, so a change to either side that breaks
//! the builder contract fails to compile, naming the generated file and the
//! item. `make contract-check` builds this crate for the host and for
//! `aarch64-unknown-none`, and its tests build each screen on a real
//! `nowayos_ui::Tree` and lay it out.
//!
//! A workspace of its own, so that Screen Compiler's own crates never depend on
//! NoWayOS.

#![no_std]

/// The product's pictures, which generated code names at the crate's root
/// (contract v15): NoWayOS's samples, which `fixtures/pictures.names` lists.
mod pictures {
    pub use nowayos_ui::pictures::samples::*;
}

include!(concat!(env!("OUT_DIR"), "/screens.rs"));

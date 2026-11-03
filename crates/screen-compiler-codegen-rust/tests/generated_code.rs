// SPDX-License-Identifier: MIT
//! The snapshots of generated Rust are compiled here, against the reference
//! contract (`contract::builder`, bound to the name `nowayos_ui::builder` that
//! generated code imports), under this workspace's lints. Running them on a
//! recording builder must build exactly the tree the IR describes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use support::{RecordError, Recorder};

/// Generated code imports `nowayos_ui::builder`; here that is the reference
/// contract.
mod nowayos_ui {
    pub use screen_compiler_codegen_rust::contract::builder;
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_welcome {
    use crate::nowayos_ui;
    include!("snapshots/welcome.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_details_form {
    use crate::nowayos_ui;
    include!("snapshots/details-form.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_edge_names {
    use crate::nowayos_ui;
    include!("snapshots/edge-names.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_parcel_status {
    use crate::nowayos_ui;
    include!("snapshots/parcel-status.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_visit_booking {
    use crate::nowayos_ui;
    include!("snapshots/visit-booking.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_queue_ticket {
    use crate::nowayos_ui;
    include!("snapshots/queue-ticket.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_check_in {
    use crate::nowayos_ui;
    include!("snapshots/check-in.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_check_in_parts {
    use crate::nowayos_ui;
    include!("snapshots/check-in-parts.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_bike_hire {
    use crate::nowayos_ui;
    include!("snapshots/bike-hire.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_table_ready {
    use crate::nowayos_ui;
    include!("snapshots/table-ready.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_form_controls {
    use crate::nowayos_ui;
    include!("snapshots/form-controls.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_parking_paid_css {
    use crate::nowayos_ui;
    include!("snapshots/parking-paid-css.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_order_summary {
    use crate::nowayos_ui;
    include!("snapshots/order-summary.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_notice_board {
    use crate::nowayos_ui;
    include!("snapshots/notice-board.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_fuelling {
    use crate::nowayos_ui;
    include!("snapshots/fuelling.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_tiles {
    use crate::nowayos_ui;
    include!("snapshots/tiles.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_order_ready {
    use crate::nowayos_ui;
    include!("snapshots/order-ready.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_parking_paid {
    use crate::nowayos_ui;
    include!("snapshots/parking-paid.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_lost_property_1_item {
    use crate::nowayos_ui;
    include!("snapshots/lost-property-1-item.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_lost_property_2_contact {
    use crate::nowayos_ui;
    include!("snapshots/lost-property-2-contact.rs");
}

#[allow(
    dead_code,
    reason = "the module is compiled to prove the generated code builds; the tests call only part of it"
)]
mod fixture_lost_property_3_review {
    use crate::nowayos_ui;
    include!("snapshots/lost-property-3-review.rs");
}

/// The fixtures compiled above; kept in step with `tests/snapshots/`.
const COMPILED: [&str; 21] = [
    "bike-hire",
    "check-in",
    "check-in-parts",
    "details-form",
    "edge-names",
    "form-controls",
    "fuelling",
    "lost-property-1-item",
    "lost-property-2-contact",
    "lost-property-3-review",
    "notice-board",
    "order-ready",
    "order-summary",
    "parking-paid",
    "parking-paid-css",
    "parcel-status",
    "queue-ticket",
    "table-ready",
    "tiles",
    "visit-booking",
    "welcome",
];

fn ir_outline(fixture: &str) -> String {
    let (module, _) = support::convert_fixture(fixture);
    assert_eq!(module.screens.len(), 1);
    screen_compiler_ir::outline(&module, &module.screens[0])
}

#[test]
fn welcome_builds_exactly_its_ir() {
    use fixture_welcome::{tokens, welcome_screen as screen};
    let mut recorder = Recorder::with_capacity(64);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("welcome.tsx"));
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    assert_eq!(recorder.describe(nodes.title), "text \"Welcome\"");
    assert_eq!(recorder.describe(nodes.start), "button \"Start\" action=1");
    assert_eq!(screen::actions::START.get(), 1);
    assert_eq!(screen::actions::HELP.get(), 2);
    assert_eq!((tokens::SPACE_SM, tokens::SPACE_LG), (8, 24));
}

/// Wrapped text, a progress bar numbered with the
/// slots, and icons, build exactly as the IR says.
#[test]
fn notice_board_and_fuelling_build_exactly_their_ir() {
    let mut recorder = Recorder::with_capacity(64);
    let nodes = fixture_notice_board::notice_board::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("notice-board.tsx"));
    let mut recorder = Recorder::with_capacity(64);
    let nodes = fixture_fuelling::fuelling::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("fuelling.tsx"));
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    let mut recorder = Recorder::with_capacity(64);
    let nodes = fixture_tiles::tiles::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("tiles.tsx"));
    use fixture_fuelling::fuelling::slots;
    assert_eq!(
        (
            slots::AMOUNT.get(),
            slots::LITRES.get(),
            slots::FUELLED.get()
        ),
        (1, 2, 3)
    );
}

#[test]
fn details_form_builds_exactly_its_ir() {
    use fixture_details_form::{details_form as screen, tokens};
    let mut recorder = Recorder::with_capacity(64);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("details-form.tsx"));
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    assert_eq!(recorder.describe(nodes.heading), "text \"Your details\"");
    assert_eq!(
        recorder.describe(nodes.full_name),
        "input field=1 max_len=64 mode=text placeholder=\"First and last name\""
    );
    assert_eq!(
        recorder.describe(nodes.email),
        "input field=2 max_len=64 mode=text placeholder=\"name@example.com\""
    );
    assert_eq!(
        recorder.describe(nodes.submit),
        "button \"Continue\" action=2"
    );
    assert_eq!(screen::actions::BACK.get(), 1);
    assert_eq!(tokens::EDGE, 32);
}

#[test]
fn edge_names_compile_and_build_exactly_their_ir() {
    // `on_2fa` keeps its `on`: `actions::2FA` would not compile.
    use fixture_edge_names::verify_step as screen;
    let mut recorder = Recorder::with_capacity(64);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("edge-names.tsx"));
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    assert_eq!(
        (
            screen::actions::ON_2FA.get(),
            screen::actions::RETRY2.get(),
            screen::actions::HTTP_CHECK.get()
        ),
        (1, 2, 3)
    );
    assert_eq!(
        recorder.describe(nodes.step_title),
        "text \"Enter the code\""
    );
    assert_eq!(
        recorder.describe(nodes.code_1),
        "input field=1 max_len=64 mode=text placeholder=\"6-digit code\""
    );
}

#[test]
fn parcel_status_builds_exactly_its_ir_with_groups_and_slots() {
    // Subset v1: conditional rendering and text from props.
    use fixture_parcel_status::parcel_status as screen;
    let mut recorder = Recorder::with_capacity(64);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(
        recorder.outline(nodes.root),
        ir_outline("parcel-status.tsx")
    );
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    // One group per prop, shared by both elements `ready` controls.
    assert_eq!(
        (screen::groups::READY.get(), screen::groups::LATE.get()),
        (1, 2)
    );
    assert_eq!(
        (
            screen::slots::LOCKER.get(),
            screen::slots::COLLECT_BY.get(),
            screen::slots::LATE_NOTICE.get()
        ),
        (1, 2, 3)
    );
    assert_eq!(recorder.describe(nodes.locker), "slot 1");
    assert_eq!(
        recorder.describe(nodes.open),
        "button \"Open the locker\" action=2"
    );
    // No text from a prop is compiled in: the slots hold none.
    let rust = include_str!("snapshots/parcel-status.rs");
    assert!(!rust.contains("lateNotice\")") && rust.contains("b.slot(slots::LATE_NOTICE)"));
}

#[test]
fn bike_hire_builds_exactly_its_ir_with_its_switch_and_its_negation() {
    // Contract v13: a prop negated, and a prop compared with literals.
    use fixture_bike_hire::bike_hire as screen;
    let mut recorder = Recorder::with_capacity(64);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("bike-hire.tsx"));
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    // One switch for the prop, its cases in the order first written.
    assert_eq!(screen::switches::STEP.get(), 1);
    assert_eq!(
        (
            screen::switches::step::CHOOSE,
            screen::switches::step::REVIEW,
            screen::switches::step::PAY
        ),
        (0, 1, 2)
    );
    assert_eq!(screen::groups::SIGNED_IN.get(), 1);
    let outline = recorder.outline(nodes.root);
    for line in [
        "switch 1 case 0",
        "switch 1 case 1",
        "switch 1 case 2",
        "unless group 1",
    ] {
        assert!(outline.contains(line), "{line}:\n{outline}");
    }
}

#[test]
fn table_ready_builds_exactly_its_ir_with_literals_around_its_slots() {
    // Contract v14: one prop between literal text, compiled in.
    use fixture_table_ready::table_ready as screen;
    let mut recorder = Recorder::with_capacity(64);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("table-ready.tsx"));
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    let outline = recorder.outline(nodes.root);
    for line in [
        "slot 1 role=heading weight=bold between \"Hello, \" and \"!\"",
        "slot 2 between \"\" and \" guests on this booking\"",
        "slot 4 role=display weight=bold between \"TABLE \" and \"\"",
    ] {
        assert!(outline.contains(line), "{line}:\n{outline}");
    }
    // Only the prop's part is the application's: its name is in no literal.
    let rust = include_str!("snapshots/table-ready.rs");
    assert!(rust.contains("b.slot_template(slots::COUNT, &ui::Template { before: \"\", after: \" guests on this booking\" })"));
}

#[test]
fn queue_ticket_builds_exactly_its_ir_with_its_text_roles() {
    // Contract v2: text in a role of the type scale, and Bold.
    use fixture_queue_ticket::queue_ticket as screen;
    let mut recorder = Recorder::with_capacity(16);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("queue-ticket.tsx"));
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    assert_eq!(
        recorder.describe(nodes.ticket),
        "slot 1 role=display weight=bold"
    );
    let rust = include_str!("snapshots/queue-ticket.rs");
    for call in [
        "b.text_styled(\"You are in the queue\", ui::TextStyle::new(ui::TextRole::Title))?",
        "b.text_styled(\"Your number\", ui::TextStyle::new(ui::TextRole::Label).bold())?",
        "b.slot_styled(slots::TICKET, ui::TextStyle::new(ui::TextRole::Display))?",
        "b.text(\"We will call your number when a desk is free.\")?",
        "b.text_styled(\"Please stay in the waiting area.\", ui::TextStyle::new(ui::TextRole::Body).bold())?",
        "b.text_styled(\"Numbers are called in order.\", ui::TextStyle::new(ui::TextRole::Caption))?",
    ] {
        assert!(rust.contains(call), "{call}");
    }
}

/// The screen built of parts - its own components and a fragment - is
/// check-in's, written flat: the same IR, and the same builder calls in the
/// same order, only the source locations in comments differ.
#[test]
fn check_in_built_of_parts_is_check_in_written_flat() {
    assert_eq!(ir_outline("check-in-parts.tsx"), ir_outline("check-in.tsx"));
    let calls = |rust: &str| -> Vec<String> {
        rust.lines()
            .filter(|line| line.trim_start().starts_with("let n"))
            .map(|line| line.split(" // ").next().unwrap_or(line).to_owned())
            .collect()
    };
    let flat = calls(include_str!("snapshots/check-in.rs"));
    assert!(!flat.is_empty());
    assert_eq!(calls(include_str!("snapshots/check-in-parts.rs")), flat);
    use fixture_check_in_parts::check_in as screen;
    let mut recorder = Recorder::with_capacity(32);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("check-in.tsx"));
}

#[test]
fn check_in_builds_exactly_its_ir_with_its_surfaces_and_variants() {
    // Contract v3: buttons in their variants, and containers on
    // surfaces.
    use fixture_check_in::check_in as screen;
    let mut recorder = Recorder::with_capacity(32);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(recorder.outline(nodes.root), ir_outline("check-in.tsx"));
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    let rust = include_str!("snapshots/check-in.rs");
    for call in [
        "b.row_on(ui::Surface::Header, ",
        "b.column_on(ui::Surface::Card, ",
        "b.column_on(ui::Surface::Notice(ui::Tone::Success), ",
        "b.column_on(ui::Surface::Notice(ui::Tone::Caution), ",
        "b.column_on(ui::Surface::Notice(ui::Tone::Warning), ",
        "b.button_with(\"Home\", actions::HOME, ui::ButtonVariant::Secondary)?",
        "b.button_with(\"Cancel\", actions::CANCEL, ui::ButtonVariant::Quiet)?",
        "b.button(\"Check in\", actions::CHECK_IN)?",
    ] {
        assert!(rust.contains(call), "{call}");
    }
}

#[test]
fn visit_booking_builds_exactly_its_ir_with_its_fields() {
    // Subset v1: text inputs with a length limit and a mode, a radio
    // group and a checkbox, each bound to a field named after its id.
    use fixture_visit_booking::{tokens, visit_booking as screen};
    let mut recorder = Recorder::with_capacity(64);
    let nodes = screen::build(&mut recorder).unwrap();
    assert_eq!(
        recorder.outline(nodes.root),
        ir_outline("visit-booking.tsx")
    );
    assert_eq!(recorder.roots(), 1, "every node is in the tree");
    assert_eq!(
        recorder.describe(nodes.name),
        format!(
            "input field=1 max_len={} mode=text placeholder=\"Your name\"",
            tokens::NAME_LEN
        )
    );
    assert_eq!(
        recorder.describe(nodes.phone),
        "input field=2 max_len=15 mode=digits placeholder=\"Phone, digits only\""
    );
    assert_eq!(
        recorder.describe(nodes.email),
        "input field=3 max_len=64 mode=email placeholder=\"Email (optional)\""
    );
    assert_eq!(
        recorder.describe(nodes.when),
        "radio_group field=4 [\"Morning\", \"Afternoon\", \"Evening\"]"
    );
    assert_eq!(
        recorder.describe(nodes.remind),
        "checkbox field=5 \"Remind me the day before\""
    );
    assert_eq!(
        (
            screen::fields::NAME.get(),
            screen::fields::PHONE.get(),
            screen::fields::EMAIL.get(),
            screen::fields::WHEN.get(),
            screen::fields::REMIND.get(),
        ),
        (1, 2, 3, 4, 5)
    );
}

#[test]
fn a_field_number_used_twice_is_the_builders_error() {
    // The recorder refuses it as NoWayOS's tree does (`DuplicateField`).
    use screen_compiler_codegen_rust::contract::builder::{FieldId, Input, UiBuilder};
    let mut recorder = Recorder::with_capacity(8);
    recorder.input(FieldId::new(1), Input::new()).unwrap();
    assert_eq!(
        recorder.checkbox(FieldId::new(1), "x"),
        Err(RecordError::Duplicate)
    );
}

#[test]
fn a_full_builder_is_an_error_not_a_panic() {
    for capacity in 0..13 {
        let mut recorder = Recorder::with_capacity(capacity);
        assert_eq!(
            fixture_details_form::details_form::build(&mut recorder),
            Err(RecordError::Full),
            "capacity {capacity}"
        );
    }
    let mut recorder = Recorder::with_capacity(13);
    assert!(fixture_details_form::details_form::build(&mut recorder).is_ok());
}

#[test]
fn every_snapshot_is_compiled_here() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let snapshots: BTreeSet<String> = fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect();
    let compiled: BTreeSet<String> = COMPILED.iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(
        snapshots, compiled,
        "add an `include!` module above for each new snapshot"
    );
}

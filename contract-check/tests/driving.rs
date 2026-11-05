// SPDX-License-Identifier: MIT
//! How an application drives a converted screen, as the website's guide shows
//! it: build it into a tree, set its slots, show or hide its groups, hand the
//! tree each input event, and read the fields when an action comes back. This
//! is that code, on `fixtures/visit-booking.tsx` as Screen Compiler converts
//! it and on NoWayOS's real UI core, so the guide's example compiles and runs
//! in `make ci`.

use nowayos_display::Rect;
use nowayos_render_eg::{Rgb888, Theme};
use nowayos_ui::{InputEvent, NodeId, Tree, UiError, UiEvent};
use screen_compiler_contract_check::snapshot_visit_booking::visit_booking::{
    self, actions, fields, groups, slots,
};

/// A booking as the application reads it from the screen's fields: the
/// name typed, the time of day chosen (from 0) and whether to remind.
type Booking = (String, Option<u8>, bool);

/// The input events of a tap on `node`, as the input broker delivers them:
/// the pointer moved over it, then its button pressed and released.
fn tap(tree: &Tree<64>, node: NodeId) -> [InputEvent; 3] {
    let at = tree
        .get(node)
        .map(|node| node.rect().center())
        .unwrap_or_default();
    [
        InputEvent::PointerMove { x: at.x, y: at.y },
        InputEvent::PointerButton {
            button: 0,
            down: true,
        },
        InputEvent::PointerButton {
            button: 0,
            down: false,
        },
    ]
}

/// Builds the screen, shows the phone number's error in its slot and group
/// and hides it again, fills the form in as a visitor would, and taps Book:
/// the error as shown, the booking the application read, and whether it was
/// told to go back.
fn book_a_visit() -> Result<(Option<String>, Option<Booking>, bool), UiError> {
    let mut tree = Tree::<64>::new();
    let screen = visit_booking::build(&mut tree)?;
    tree.set_root(screen.root)?;

    // A slot is text the application sets; a group is content it shows or hides.
    tree.set_slot(slots::PHONE_ERROR, "Digits only, please")?;
    tree.set_visible(groups::PHONE_ERROR, true)?;
    let shown = tree
        .visible(groups::PHONE_ERROR)
        .filter(|visible| *visible)
        .and(tree.slot_text(slots::PHONE_ERROR))
        .map(str::to_owned);
    tree.set_visible(groups::PHONE_ERROR, false)?;

    // What a visitor enters, as if typed and tapped.
    tree.set_value(fields::NAME, "Sam")?;
    tree.set_selected(fields::WHEN, Some(1))?;
    tree.set_checked(fields::REMIND, true)?;

    // Every input event goes to the tree; a tap on Book comes back as its action.
    tree.layout(
        Rect::new(0, 0, 800, 480),
        &Theme::<Rgb888>::light().metrics(),
    )?;
    let mut booked = None;
    let mut back = false;
    for event in tap(&tree, screen.book) {
        match tree.handle(event) {
            Some(UiEvent::Action(actions::BOOK)) => {
                let name = tree.value(fields::NAME).unwrap_or("");
                let when = tree.selected(fields::WHEN);
                let remind = tree.checked(fields::REMIND).unwrap_or(false);
                booked = Some((name.to_owned(), when, remind));
            }
            Some(UiEvent::Action(actions::BACK)) => back = true,
            _ => {}
        }
    }
    Ok((shown, booked, back))
}

#[test]
fn the_guide_drives_visit_booking() {
    assert_eq!(
        book_a_visit(),
        Ok((
            Some("Digits only, please".to_owned()),
            Some(("Sam".to_owned(), Some(1), true)),
            false
        ))
    );
}

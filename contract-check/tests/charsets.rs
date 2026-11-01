// SPDX-License-Identifier: MIT
//! Screen Compiler refuses text a role's glyph atlases do not draw, using
//! its own copy of NoWayOS's character sets (`screen_compiler_ir::is_printable`
//! and `is_display`), so that its crates never depend on NoWayOS. This test
//! holds the copy to NoWayOS's: a change on either side fails here.
//!
//! The copy's module, `screen-compiler-ir`'s `style.rs`, is compiled into this
//! test as it stands, by path: it depends on nothing, and a local crate in
//! this workspace's graph would need the baseline's local-crate allowance.

#[allow(
    dead_code,
    reason = "the whole module is compiled; this test uses its character sets and roles"
)]
#[path = "../../crates/screen-compiler-ir/src/style.rs"]
mod screen_compiler_style;

use nowayos_ui::TextRole;
use nowayos_ui::text::{is_display, is_printable};

/// NoWayOS's role for each of Screen Compiler's.
fn nowayos_role(role: screen_compiler_style::TextRole) -> TextRole {
    match role {
        screen_compiler_style::TextRole::Display => TextRole::Display,
        screen_compiler_style::TextRole::Title => TextRole::Title,
        screen_compiler_style::TextRole::Heading => TextRole::Heading,
        screen_compiler_style::TextRole::Body => TextRole::Body,
        screen_compiler_style::TextRole::Label => TextRole::Label,
        screen_compiler_style::TextRole::Caption => TextRole::Caption,
    }
}

#[test]
fn screen_compiler_character_sets_are_nowayos_ones() {
    for c in (0..=0x0010_FFFF_u32).filter_map(char::from_u32) {
        assert_eq!(
            screen_compiler_style::is_printable(c),
            is_printable(c),
            "{c:?}"
        );
        assert_eq!(screen_compiler_style::is_display(c), is_display(c), "{c:?}");
    }
}

#[test]
fn every_role_draws_what_nowayos_lets_it_show() {
    let mut text = [0_u8; 4];
    for role in screen_compiler_style::TextRole::ALL {
        let style = nowayos_ui::TextStyle::new(nowayos_role(role));
        for c in (0..=0x2FFF_u32).filter_map(char::from_u32) {
            let text: &'static str =
                Box::leak(c.encode_utf8(&mut text).to_owned().into_boxed_str());
            let shown = nowayos_ui::NodeKind::Text(text)
                .check_text_in(&style)
                .is_ok();
            assert_eq!(role.draws(c), shown, "{role}: {c:?}");
        }
    }
}

/// Screen Compiler's icon names are NoWayOS's icons', in order: an icon
/// added or renamed on either side fails here.
#[test]
fn screen_compiler_icon_names_are_nowayos_ones() {
    let nowayos: Vec<&str> = nowayos_ui::icons::ALL
        .iter()
        .map(|icon| icon.name())
        .collect();
    assert_eq!(screen_compiler_style::ICON_NAMES.to_vec(), nowayos);
}

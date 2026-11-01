// SPDX-License-Identifier: MIT
//! The parser front end must never crash, whatever it is given.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use screen_compiler_ir::special_char;
use screen_compiler_ir::{Code, Loc};
use screen_compiler_tsx::syntax::{Attr, AttrValue, Child, Expr, Item, VarDecl};
use screen_compiler_tsx::{MAX_SOURCE_BYTES, parse};

/// `prefix`, then `open` repeated, then `middle`, then `close` repeated, as
/// long as fits in the source limit.
fn nested(prefix: &str, open: &str, middle: &str, close: &str, suffix: &str) -> String {
    let fixed = prefix.len() + middle.len() + suffix.len();
    let n = (MAX_SOURCE_BYTES - fixed) / (open.len() + close.len());
    let mut s = String::with_capacity(MAX_SOURCE_BYTES);
    s.push_str(prefix);
    for _ in 0..n {
        s.push_str(open);
    }
    s.push_str(middle);
    for _ in 0..n {
        s.push_str(close);
    }
    s.push_str(suffix);
    assert!(s.len() <= MAX_SOURCE_BYTES);
    s
}

/// The heaviest nesting shapes measured at the size limit (by stack used in
/// a debug build), all of which reach the parser: none uses brackets, or each
/// hides its closing bracket in a comment or string so the raw bracket count
/// stays low. If a parser upgrade grows its stack frames past what
/// `PARSER_STACK_BYTES` allows, this test aborts instead of a user's run.
fn heavy_shapes() -> Vec<(&'static str, String)> {
    vec![
        // Type arguments: about 101 MiB of stack in a debug build (arm64).
        (
            "type arguments, unclosed",
            nested("let x: ", "A<", "", "", ""),
        ),
        ("type arguments", nested("let x: ", "A<", "B", ">", " = 1;")),
        // One byte per level, no brackets: about 68 MiB.
        ("! chain", nested("const x = ", "!", "1", "", ";")),
        ("~ chain", nested("const x = ", "~", "1", "", ";")),
        // Brackets whose closing partner hides in a string or comment.
        (
            "( with ')' strings",
            nested("const x = ", "(')'", "", "", ""),
        ),
        (
            "( with comments",
            nested("const x = ", "(/*)*/", "1", ")", ";"),
        ),
        (
            "calls with comments",
            nested("const x = ", "f(/*)*/", "1", ")", ";"),
        ),
        (
            "arrays with comments",
            nested("const x = ", "[/*]*/", "1", "]", ";"),
        ),
        (
            "objects with comments",
            nested("const x = ", "{a:/*}*/", "1", "}", ";"),
        ),
        (
            "block arrows",
            nested("const x = ", "()=>{/*}*/", "", "}", ";"),
        ),
        (
            "class heritage",
            nested("const x = ", "class extends (/*)*/", "A", "){}", ";"),
        ),
        (
            "template literals",
            nested("const x = ", "`${/*}*/", "1", "}`", ";"),
        ),
        (
            "JSX attributes",
            nested("const x = ", "<a b={/*}*/", "", "} />", ";"),
        ),
        // Other recursive shapes without brackets.
        ("new chain", nested("const x = ", "new ", "A", "", ";")),
        ("arrow chain", nested("const x = ", "a=>", "a", "", ";")),
        ("function types", nested("let x: ", "()=>", "A", "", ";")),
        (
            "conditional types",
            nested("type T = ", "A extends B ? C : ", "D", "", ";"),
        ),
        ("ternaries", nested("const x = ", "a ? a : ", "a", "", ";")),
        ("labels", nested("", "a:", "", "", ";")),
        ("await chain", nested("", "await ", "1", "", ";")),
        ("JSX", nested("const x = ", "<a>", "", "</a>", ";")),
        ("JSX, unclosed", nested("const x = ", "<a>", "", "", "")),
    ]
}

#[test]
fn deeply_nested_sources_at_the_size_limit_do_not_crash() {
    for (name, source) in heavy_shapes() {
        // Parsing may succeed or report problems; it must return either way.
        let result = parse(&source);
        let diagnostics = match &result {
            Ok(module) => &module.diagnostics,
            Err(diagnostics) => diagnostics,
        };
        assert!(
            !diagnostics
                .iter()
                .any(|d| d.message.contains("brackets are open")),
            "{name}: rejected by the bracket count, so the parser never saw it"
        );
    }
}

#[test]
fn deep_jsx_is_reported_as_a_limit_not_converted() {
    let source = nested("const x = ", "<a>", "", "</a>", ";");
    let module = parse(&source).unwrap();
    assert!(module.diagnostics.iter().any(|d| d.code == Code::Limit));
}

#[test]
fn deep_brackets_are_rejected_before_parsing() {
    let source = nested("const x = ", "(", "1", ")", ";");
    let diagnostics = parse(&source).unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, Code::Limit);
}

#[test]
fn unclosed_runs_of_brackets_never_reach_the_parser() {
    // One byte per level: at the size limit these need 177 to 226 MiB of
    // parser stack in a debug build, close to `PARSER_STACK_BYTES`. The raw
    // bracket count is what keeps them out.
    for open in ["(", "[", "{"] {
        let source = open.repeat(MAX_SOURCE_BYTES);
        let diagnostics = parse(&source).unwrap_err();
        assert_eq!(diagnostics.len(), 1, "{open}");
        assert_eq!(diagnostics[0].code, Code::Limit, "{open}");
        assert_eq!(diagnostics[0].loc, Some(Loc::new(1, 257)), "{open}");
    }
}

fn config() -> Config {
    Config {
        cases: 256,
        failure_persistence: None,
        rng_seed: RngSeed::Fixed(0x7473_7872_6f62_7573),
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn arbitrary_tsx_like_input_never_panics(
        source in "[<>/{}()\\[\\]=\"'`a-zA-Z0-9 \\n\\r\\t.;:,&#!?+*\\\\\u{0}\u{1b}\u{7f}\u{85}\u{a0}\u{e9}\u{200b}\u{202e}\u{2028}\u{2029}\u{feff}\u{1f600}-]{0,160}"
    ) {
        let _ = parse(&source);
    }

    #[test]
    fn arbitrary_jsx_fragments_never_panic(
        body in "(<[A-Za-z]{1,3}( [a-z]{1,3}=(\"[^\"]{0,5}\"|\\{[a-z0-9]{0,3}\\}))*>|</[A-Za-z]{1,3}>|<>|</>|\\{[^{}]{0,6}\\}|[a-z &;#\\n\\r\\t\u{0}\u{1b}\u{a0}\u{e9}\u{200b}\u{202e}\u{2028}\u{feff}]{0,6}|\r\n){0,20}"
    ) {
        let _ = parse(&format!("const x = <A>{body}</A>;"));
    }

    #[test]
    fn converted_text_and_attribute_strings_never_hold_special_characters(
        body in "(<T( a=\"[a-z &#;0-9x\\n\\t\u{1b}\u{202e}\u{2028}]{0,6}\")?>[a-z &#;0-9x\\n\\r\\t\u{1b}\u{a0}\u{200b}\u{202e}\u{2028}]{0,8}</T>){0,6}"
    ) {
        if let Ok(module) = parse(&format!("const x = <A>{body}</A>;")) {
            let mut stack: Vec<&Child> = Vec::new();
            for item in &module.items {
                if let Item::Var(VarDecl { init: Some(Expr::Element(e)), .. }) = item {
                    stack.extend(e.children.iter());
                }
            }
            while let Some(child) = stack.pop() {
                match child {
                    Child::Text { value, .. } => {
                        prop_assert!(value.chars().all(|c| special_char(c).is_none()), "{:?}", value);
                    }
                    Child::Element(e) => {
                        for attr in &e.attrs {
                            if let Attr::Named { value: AttrValue::Str { value, .. }, .. } = attr {
                                prop_assert!(value.chars().all(|c| special_char(c).is_none()), "{:?}", value);
                            }
                        }
                        stack.extend(e.children.iter());
                    }
                    _ => {}
                }
            }
        }
    }
}

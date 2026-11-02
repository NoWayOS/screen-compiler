// SPDX-License-Identifier: MIT
//! TSX parsing for Screen Compiler.
//!
//! [`parse`] turns TSX source into Screen Compiler's own syntax tree
//! ([`syntax::SourceModule`]) using `oxc_parser`. This is the only crate that
//! depends on oxc, so a parser upgrade touches nothing else.
//!
//! Hardening, because the parser is recursive and has no nesting limit of its
//! own:
//!
//! * sources over [`MAX_SOURCE_BYTES`] are rejected before parsing, which
//!   bounds how deep anything can nest;
//! * a source in which more than [`MAX_BRACKET_DEPTH`] brackets are open at
//!   once is rejected before parsing. The count is taken on the raw text, so
//!   brackets inside strings, comments and JSX text count too, and a closing
//!   bracket anywhere closes one: it is **not** a bound on the parser's
//!   recursion. What it does is make every level of bracket nesting that
//!   reaches the parser cost several bytes (a closing bracket hidden in a
//!   comment or string) instead of one. Unbalanced runs of `(` or `[` at the
//!   size limit need 177 to 226 MiB of stack in a debug build (measured on
//!   arm64); the cheapest shapes that get past the check need under 50 MiB;
//! * parsing runs on its own thread with a [`PARSER_STACK_BYTES`] stack. The
//!   deepest shape measured at the size limit that reaches the parser (nested
//!   `A<` type arguments) needs about 101 MiB in a debug build, and the
//!   robustness tests parse it and the other heavy shapes at the limit, so a
//!   parser upgrade that grows its stack frames fails the tests rather than
//!   aborting on a user's file.
//!
//! These bound stack use. They do not bound time and memory against every
//! pathological input: oxc 0.133 retries some constructs speculatively
//! (type arguments after a name, as in `a<a<a<…`, and arrow functions inside
//! conditionals) and keeps what each failed attempt allocated, so time and
//! memory grow with the square of that nesting (about 13 KB of nested `a<`
//! takes 5 GB). Screen Compiler is meant for screens a team writes, not for
//! untrusted input.
//!
//! Syntax errors are returned as diagnostics with line and column; nothing is
//! guessed from a source that does not parse.
//!
//! [`census()`] parses the same way and counts every construct written, for
//! the coverage report ([`census`](mod@census)).

#![deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)]

pub mod census;
mod convert;
pub mod jsx_text;
mod lines;
pub mod syntax;

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::SourceType;
use screen_compiler_ir::{Code, Diagnostic, sort_diagnostics};

pub use lines::LineIndex;
pub use syntax::SourceModule;

/// The largest source accepted, in bytes. Screens are far smaller; the limit
/// keeps parsing time and stack depth bounded.
pub const MAX_SOURCE_BYTES: usize = 64 * 1024;

/// The most brackets (`(`, `[`, `{`) that may be open at once, counted on the
/// raw text: a coarse early reject, not a bound on the parser's recursion
/// (see the crate documentation).
pub const MAX_BRACKET_DEPTH: usize = 256;

/// Stack size of the parser thread. Only the pages used are committed.
pub const PARSER_STACK_BYTES: usize = 256 * 1024 * 1024;

/// Parses TSX `source`.
///
/// Returns the syntax tree when the source parses. Problems that leave the
/// tree usable, such as JSX text that cannot be converted exactly, are in
/// [`SourceModule::diagnostics`]; the analyser reports them together with its
/// own findings and refuses to produce IR if any of them is an error.
///
/// # Errors
///
/// Returns the diagnostics (sorted by position) when the source is too large
/// or too deeply nested, or does not parse.
pub fn parse(source: &str) -> Result<SourceModule, Vec<Diagnostic>> {
    on_parser_thread(source, |program| {
        let mut module = convert::Converter::new(source).module(program);
        sort_diagnostics(&mut module.diagnostics);
        module
    })
}

/// Counts every construct written in TSX `source`, for the coverage report:
/// what is written, not whether it converts.
///
/// # Errors
///
/// As [`parse`]: the diagnostics when the source is too large or too deeply
/// nested, or does not parse.
pub fn census(source: &str) -> Result<census::Census, Vec<Diagnostic>> {
    on_parser_thread(source, census::count)
}

/// Parses `source` on a thread with [`PARSER_STACK_BYTES`] of stack, after
/// the size and bracket checks, and makes `make`'s result of its program.
fn on_parser_thread<T: Send>(
    source: &str,
    make: impl FnOnce(&Program<'_>) -> T + Send,
) -> Result<T, Vec<Diagnostic>> {
    precheck(source)?;
    let result = std::thread::scope(|scope| {
        let spawned = std::thread::Builder::new()
            .name("screen-compiler-parse".to_owned())
            .stack_size(PARSER_STACK_BYTES)
            .spawn_scoped(scope, || parse_here(source, make));
        match spawned {
            Ok(handle) => handle.join().unwrap_or_else(|_| {
                Err(vec![Diagnostic::error(
                    Code::Internal,
                    None,
                    "the TSX parser stopped unexpectedly; please report this input",
                )])
            }),
            Err(error) => Err(vec![Diagnostic::error(
                Code::Internal,
                None,
                format!("could not start the parser thread: {error}"),
            )]),
        }
    });
    result.map_err(|mut diagnostics| {
        sort_diagnostics(&mut diagnostics);
        diagnostics
    })
}

fn precheck(source: &str) -> Result<(), Vec<Diagnostic>> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(vec![
            Diagnostic::error(
                Code::Limit,
                None,
                format!(
                    "the source is {} bytes; the limit is {MAX_SOURCE_BYTES}",
                    source.len()
                ),
            )
            .with_help("split the file: one screen per file is plenty"),
        ]);
    }
    let mut depth = 0_usize;
    for (offset, byte) in source.bytes().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => {
                depth = depth.saturating_add(1);
                if depth > MAX_BRACKET_DEPTH {
                    let loc = LineIndex::new(source).loc(source, offset);
                    return Err(vec![
                        Diagnostic::error(
                            Code::Limit,
                            Some(loc),
                            format!("more than {MAX_BRACKET_DEPTH} brackets are open here"),
                        )
                        .with_help(
                            "brackets are counted on the raw text, including any inside strings, comments and JSX text",
                        ),
                    ]);
                }
            }
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    Ok(())
}

fn parse_here<T>(source: &str, make: impl FnOnce(&Program<'_>) -> T) -> Result<T, Vec<Diagnostic>> {
    let allocator = Allocator::default();
    let source_type = SourceType::tsx().with_module(true);
    let options = ParseOptions {
        preserve_parens: false,
        ..ParseOptions::default()
    };
    let parsed = Parser::new(&allocator, source, source_type)
        .with_options(options)
        .parse();
    let lines = LineIndex::new(source);
    if parsed.panicked || !parsed.errors.is_empty() {
        let mut diagnostics: Vec<Diagnostic> = parsed
            .errors
            .iter()
            .map(|error| {
                let offset = error
                    .labels
                    .iter()
                    .flatten()
                    .find(|label| label.primary())
                    .or_else(|| error.labels.iter().flatten().next())
                    .map(|label| label.offset());
                let mut diagnostic = Diagnostic::error(
                    Code::Syntax,
                    offset.map(|o| lines.loc(source, o)),
                    error.message.to_string(),
                );
                if let Some(help) = &error.help {
                    diagnostic = diagnostic.with_help(help.to_string());
                }
                diagnostic
            })
            .collect();
        if diagnostics.is_empty() {
            diagnostics.push(Diagnostic::error(
                Code::Syntax,
                None,
                "the source could not be parsed",
            ));
        }
        return Err(diagnostics);
    }
    Ok(make(&parsed.program))
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing,
        reason = "test code: a failed check panics, and tests index and compute on known data"
    )]

    use super::*;
    use crate::syntax::*;
    use screen_compiler_ir::Loc;

    fn parse_ok(source: &str) -> SourceModule {
        match parse(source) {
            Ok(module) => module,
            Err(diagnostics) => panic!("{diagnostics:#?}"),
        }
    }

    fn only_function(module: &SourceModule) -> &FunctionDecl {
        match module.items.as_slice() {
            [Item::Function(f)] => f,
            other => panic!("expected one function, got {other:#?}"),
        }
    }

    fn returned_element(f: &Function) -> &Element {
        match &f.body {
            Body::Block(statements) => match statements.as_slice() {
                [
                    Stmt::Return {
                        value: Some(Expr::Element(e)),
                        ..
                    },
                ] => e,
                other => panic!("expected `return <…>`, got {other:#?}"),
            },
            Body::Expr(e) => match e.as_ref() {
                Expr::Element(e) => e,
                other => panic!("expected an element, got {other:#?}"),
            },
        }
    }

    #[test]
    fn converts_a_component_with_positions_and_jsx_semantics() {
        let module = parse_ok(
            "export function Hello({ onGo, title: heading }: P) {\n  return (\n    <Column gap={8} id=\"main\">\n      Hi &amp; welcome\n      <Button onPress={onGo}>Go</Button>\n    </Column>\n  );\n}\n",
        );
        let f = only_function(&module);
        assert_eq!(f.name.as_deref(), Some("Hello"));
        assert_eq!(f.export, Export::Named);
        match &f.function.params {
            Params::Destructured(props) => {
                assert_eq!(props.len(), 2);
                assert_eq!(
                    (props[1].key.as_str(), props[1].local.as_str()),
                    ("title", "heading")
                );
            }
            other => panic!("{other:?}"),
        }
        let column = returned_element(&f.function);
        assert_eq!(column.name, ElementName::Component("Column".into()));
        assert_eq!(column.loc, Loc::new(3, 5));
        match &column.attrs[1] {
            Attr::Named {
                name,
                value: AttrValue::Str { value, loc },
                ..
            } => {
                assert_eq!((name.as_str(), value.as_str()), ("id", "main"));
                assert_eq!(*loc, Loc::new(3, 24), "the opening quote");
            }
            other => panic!("{other:?}"),
        }
        match &column.children[..] {
            [Child::Text { value, loc }, Child::Element(button)] => {
                assert_eq!(value, "Hi & welcome");
                assert_eq!(*loc, Loc::new(4, 7));
                assert_eq!(button.loc, Loc::new(5, 7));
            }
            other => panic!("{other:#?}"),
        }
    }

    #[test]
    fn jsx_attribute_strings_keep_backslashes() {
        let module = parse_ok("const x = <T a=\"C:\\dir &lt;\" />;");
        match &module.items[..] {
            [Item::Var(v)] => match &v.init {
                Some(Expr::Element(e)) => match &e.attrs[..] {
                    [
                        Attr::Named {
                            value: AttrValue::Str { value, .. },
                            ..
                        },
                    ] => assert_eq!(value, "C:\\dir <"),
                    other => panic!("{other:?}"),
                },
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn describes_what_it_does_not_model() {
        let module = parse_ok(
            "class A {}\nenum E { X }\nexport default () => <Screen />;\nconst [a] = b;\nfunction f() { const [s, set] = useState(0); if (s) {} }\n",
        );
        let kinds: Vec<String> = module
            .items
            .iter()
            .map(|item| match item {
                Item::Other(o) => o.kind.clone(),
                Item::ExportDefaultExpr(e) => format!("default {}", e.describe()),
                Item::Function(f) => match &f.function.body {
                    Body::Block(stmts) => stmts
                        .iter()
                        .map(|s| match s {
                            Stmt::Other(o) => o.kind.clone(),
                            Stmt::Return { .. } => "return".into(),
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    Body::Expr(_) => "expr".into(),
                },
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                "class declaration",
                "enum declaration",
                "default a function",
                "destructuring declaration",
                "React hook `useState`, `if` statement",
            ]
        );
    }

    #[test]
    fn type_only_declarations_are_marked() {
        let module = parse_ok(
            "import type { A } from \"./a\";\ntype P = { a: string };\ninterface Q { b: number }\ndeclare const C: number;\nexport type { P };\n",
        );
        assert!(matches!(
            &module.items[0],
            Item::Import(Import {
                type_only: true,
                ..
            })
        ));
        assert!(
            module.items[1..]
                .iter()
                .all(|i| matches!(i, Item::TypeOnly { .. }))
        );
    }

    #[test]
    fn syntax_errors_are_reported_with_positions() {
        let diagnostics = parse("const x = <Column>\n  <Text>hi</Column>;\n").unwrap_err();
        assert!(!diagnostics.is_empty());
        assert!(diagnostics.iter().all(|d| d.code == Code::Syntax));
        assert!(diagnostics.iter().any(|d| d.loc.is_some()));
    }

    #[test]
    fn unsupported_character_references_are_reported_where_they_are() {
        let diagnostics = parse("const x = <Text>\n  (c) &copy; 2026\n</Text>;")
            .unwrap()
            .diagnostics;
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, Code::Unsupported);
        assert_eq!(diagnostics[0].loc, Some(Loc::new(2, 7)));
    }

    #[test]
    fn legacy_octal_escapes_in_strings_are_syntax_errors() {
        let module = parse_ok(
            "import { Text } from \"nowayos:kit\";\nconst a = <Text>{\"a\\01b\"}</Text>;\nconst b = \"\\8\";\nconst c = \"\\0\";\nconst d = \"\\\\8\";\nimport { X } from \"m\\151\";\nfunction f({ \"on\\7\": g }) {}\n",
        );
        let found: Vec<(Code, Option<Loc>)> =
            module.diagnostics.iter().map(|d| (d.code, d.loc)).collect();
        assert_eq!(
            found,
            [
                (Code::Syntax, Some(Loc::new(2, 20))),
                (Code::Syntax, Some(Loc::new(3, 12))),
                (Code::Syntax, Some(Loc::new(6, 21))),
                (Code::Syntax, Some(Loc::new(7, 17))),
            ]
        );
        assert!(module.diagnostics[0].message.contains("`\\01`"));
    }

    #[test]
    fn export_lists_and_type_declarations_carry_their_names() {
        let module = parse_ok(
            "type P = {};\ninterface Q {}\ndeclare const C: number;\nexport { S as default, P, type Q };\nexport default S;\nfunction f(): void;\nexport type { P as R };\n",
        );
        let summary: Vec<String> = module
            .items
            .iter()
            .map(|item| match item {
                Item::TypeOnly { names, .. } => format!("type {}", names.join(",")),
                Item::ExportList { specifiers, .. } => format!(
                    "export {}",
                    specifiers
                        .iter()
                        .map(|s| format!("{} as {}", s.local, s.exported))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
                Item::ExportDefaultExpr(Expr::Ident { name, .. }) => format!("default {name}"),
                other => format!("{other:?}"),
            })
            .collect();
        assert_eq!(
            summary,
            [
                "type P",
                "type Q",
                "type C",
                "export S as default,P as P",
                "default S",
                "type f",
                "type ",
            ]
        );
    }

    #[test]
    fn text_that_cannot_be_converted_is_marked_not_guessed() {
        let module =
            parse_ok("const x = <T a=\"x&copy;\" b=\"\u{202e}\">\n  (c) &copy; 2026\n</T>;");
        assert_eq!(module.diagnostics.len(), 3, "{:#?}", module.diagnostics);
        let [Item::Var(v)] = module.items.as_slice() else {
            panic!("{:#?}", module.items)
        };
        let Some(Expr::Element(e)) = &v.init else {
            panic!("{v:#?}")
        };
        assert!(matches!(
            e.attrs.as_slice(),
            [
                Attr::Named {
                    value: AttrValue::InvalidStr { .. },
                    ..
                },
                Attr::Named {
                    value: AttrValue::InvalidStr { .. },
                    ..
                },
            ]
        ));
        assert_eq!(
            e.children,
            [Child::InvalidText {
                loc: Loc::new(2, 3)
            }]
        );
    }

    #[test]
    fn oversized_and_overnested_sources_are_rejected_before_parsing() {
        let big = "a".repeat(MAX_SOURCE_BYTES + 1);
        assert_eq!(parse(&big).unwrap_err()[0].code, Code::Limit);
        let deep = format!("const x = {}1{};", "(".repeat(300), ")".repeat(300));
        let diagnostics = parse(&deep).unwrap_err();
        assert_eq!(diagnostics[0].code, Code::Limit);
        assert_eq!(diagnostics[0].loc, Some(Loc::new(1, 267)));
    }
}

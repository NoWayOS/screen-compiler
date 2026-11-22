// SPDX-License-Identifier: MIT
//! What the analyser's test files share: a source analysed, its module or
//! outline when it converts, its errors when it does not.

#![allow(
    dead_code,
    reason = "shared by several test binaries, each of which uses only part of it"
)]

use screen_compiler_analyse::{Analysis, Options, analyse};
use screen_compiler_ir::{Diagnostic, Module, outline};

/// `source` parsed and analysed with `options`; a parse error is the analysis's diagnostics.
pub fn run_with(source: &str, options: &Options) -> Analysis {
    match screen_compiler_tsx::parse(source) {
        Ok(parsed) => analyse(&parsed, options),
        Err(diagnostics) => Analysis {
            module: None,
            diagnostics,
            imported: Vec::new(),
        },
    }
}

/// `source` analysed as `test.tsx`.
pub fn run(source: &str) -> Analysis {
    run_with(source, &Options::new("test.tsx"))
}

/// The module, or a panic listing the diagnostics.
pub fn ok(source: &str) -> Module {
    let analysis = run(source);
    match analysis.module {
        Some(module) => module,
        None => panic!(
            "expected a module, got {:#?}\n{source}",
            analysis.diagnostics
        ),
    }
}

/// The outline of the file's only screen.
pub fn ok_outline(source: &str) -> String {
    let module = ok(source);
    assert_eq!(module.screens.len(), 1, "{source}");
    outline(&module, &module.screens[0])
}

/// Every error, and a check that no IR was produced.
pub fn errors(source: &str) -> Vec<Diagnostic> {
    let analysis = run(source);
    assert!(
        analysis.module.is_none(),
        "an error must stop conversion:\n{source}"
    );
    let errors = analysis.diagnostics;
    assert!(!errors.is_empty(), "expected errors\n{source}");
    errors
}

/// The one error `source` gives.
pub fn error(source: &str) -> Diagnostic {
    let mut all = errors(source);
    assert_eq!(all.len(), 1, "{all:#?}\n{source}");
    all.remove(0)
}

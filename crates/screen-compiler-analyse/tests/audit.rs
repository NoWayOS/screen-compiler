// SPDX-License-Identifier: MIT
//! The coverage report (`screen-compiler audit`): the report on `fixtures/`
//! matches `tests/snapshots/audit-fixtures.txt`, every construct of a
//! fixture that converts is judged in the subset, and a construct the
//! subset does not know is never.
//!
//! Set `SCREEN_COMPILER_BLESS=1` to rewrite the report after a deliberate
//! change (`make bless`), then review the diff.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use std::fs;
use std::path::{Path, PathBuf};

use screen_compiler_analyse::audit::{
    FileResult, Step, Verdict, audit_dir, audit_source, verdict, verdict_in,
};
use screen_compiler_tsx::census::{Construct, ImportKind};

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn fixtures() -> PathBuf {
    manifest_dir().join("../../fixtures")
}

#[test]
fn the_fixtures_report_is_as_approved() {
    let report = audit_dir(&fixtures()).unwrap().render();
    let approved = manifest_dir().join("tests/snapshots/audit-fixtures.txt");
    if std::env::var_os("SCREEN_COMPILER_BLESS").is_some() {
        fs::write(&approved, &report).unwrap();
        return;
    }
    let expected = fs::read_to_string(&approved).unwrap_or_default();
    assert!(
        expected == report,
        "the report on fixtures/ changed; if on purpose, run `make bless` and review the diff:\n{report}"
    );
}

#[test]
fn every_construct_of_a_converting_fixture_is_in_the_subset() {
    let report = audit_dir(&fixtures()).unwrap();
    let mut converting = 0;
    for (path, result) in &report.files {
        if let FileResult::Read {
            converts: true,
            census,
        } = result
        {
            converting += 1;
            for construct in census.counts.keys() {
                assert_eq!(
                    verdict_in(construct, &census.own_components),
                    Verdict::Subset,
                    "{path} converts, yet {construct:?} is judged off the subset"
                );
            }
        }
    }
    assert!(converting > 0, "no fixture converts");
}

#[test]
fn what_the_subset_does_not_know_is_never_counted_as_converting() {
    let source = r#"
import { fetchThings } from "some-package";
export function Odd() {
  const [n, setN] = useState(0);
  return <table className="x" style={{ gridTemplateColumns: "1fr" }}>{n}</table>;
}
"#;
    let FileResult::Read { converts, census } = audit_source(source, "odd.tsx") else {
        panic!("odd.tsx does not parse");
    };
    assert!(!converts);
    let judged = |construct: Construct| verdict(&construct);
    assert_eq!(judged(Construct::Html("table".into())), Verdict::Unknown);
    assert_eq!(
        judged(Construct::Style {
            element: "table".into(),
            property: "gridTemplateColumns".into()
        }),
        Verdict::Unknown
    );
    assert!(matches!(
        judged(Construct::Hook("useState".into())),
        Verdict::Out(_)
    ));
    assert_eq!(
        judged(Construct::Prop {
            element: "table".into(),
            prop: "className".into()
        }),
        Verdict::Road(Step::CssModules)
    );
    for construct in census.counts.keys() {
        assert_ne!(verdict(construct), Verdict::Subset, "{construct:?}");
    }
}

#[test]
fn the_road_takes_the_constructs_its_steps_name() {
    let road = |construct: Construct| verdict(&construct);
    // A component from a package, or from outside the screen's folder; the
    // module's own, and those it imports from beside it, are expanded.
    assert!(matches!(
        road(Construct::Component("PriceCard".into())),
        Verdict::Out(_)
    ));
    assert_eq!(road(Construct::Import(ImportKind::Local)), Verdict::Subset);
    assert!(matches!(
        road(Construct::Import(ImportKind::Outside)),
        Verdict::Out(_)
    ));
    let own = std::collections::BTreeSet::from(["PriceCard".to_owned()]);
    for construct in [
        Construct::Fragment,
        Construct::Component("PriceCard".into()),
        Construct::Prop {
            element: "PriceCard".into(),
            prop: "price".into(),
        },
    ] {
        assert_eq!(
            verdict_in(&construct, &own),
            Verdict::Subset,
            "{construct:?}"
        );
    }
    // A list with a most converts; one with none is out: the screen must
    // know how many rows it may show.
    assert_eq!(road(Construct::BoundedMap), Verdict::Subset);
    assert!(matches!(road(Construct::Map), Verdict::Out(_)));
    // Text mixing literals with a prop converts (one prop per text).
    assert_eq!(road(Construct::MixedText), Verdict::Subset);
    assert_eq!(road(Construct::Template), Verdict::Subset);
    // A prop negated, or `===` a string literal (a switch's case), converts;
    // other comparisons and statements are the application's.
    assert_eq!(road(Construct::Unary("!")), Verdict::Subset);
    assert_eq!(road(Construct::Binary("===")), Verdict::Subset);
    assert!(matches!(road(Construct::Binary("<")), Verdict::Out(_)));
    assert!(matches!(road(Construct::Statement("if")), Verdict::Out(_)));
    assert_eq!(
        road(Construct::Style {
            element: "Column".into(),
            property: "flexGrow".into()
        }),
        Verdict::Road(Step::CssLayout)
    );
    // React DOM's name for a press, and a function that only calls a prop.
    assert_eq!(
        road(Construct::Prop {
            element: "Button".into(),
            prop: "onClick".into()
        }),
        Verdict::Subset
    );
    assert_eq!(
        road(Construct::InlineFunction {
            element: "Button".into(),
            prop: "onClick".into(),
            only_calls: true
        }),
        Verdict::Subset
    );
    assert!(matches!(
        road(Construct::InlineFunction {
            element: "Button".into(),
            prop: "onClick".into(),
            only_calls: false
        }),
        Verdict::Out(_)
    ));
    assert_eq!(
        road(Construct::Html("div".into())),
        Verdict::Road(Step::HtmlElements)
    );
    assert_eq!(road(Construct::Component("Column".into())), Verdict::Subset);
}

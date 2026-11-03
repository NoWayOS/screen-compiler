// SPDX-License-Identifier: MIT
//! A screen's own components imported from other files: each `.tsx` file in
//! the screen's folder or below that it imports is read once, every name is
//! resolved in the file it is written in, and an error inside an imported
//! file is reported there.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "test code: a failed check panics, and tests index and compute on known data"
)]

use std::collections::BTreeMap;

use screen_compiler_analyse::files::{Load, LoadError, MAX_FILES};
use screen_compiler_analyse::{Analysis, Options, analyse, analyse_with};
use screen_compiler_ir::{Code, Diagnostic, Loc, Module, outline};

const KIT: &str = "import { Button, Column, Row, Screen, Text } from \"nowayos:kit\";\n";

/// Files in memory, by path from the screen's folder; and each path read.
#[derive(Default)]
struct Sources {
    files: BTreeMap<String, String>,
    read: Vec<String>,
}

impl Load for Sources {
    fn load(&mut self, path: &str) -> Result<String, LoadError> {
        self.read.push(path.to_owned());
        self.files.get(path).cloned().ok_or(LoadError::Missing)
    }
}

fn sources(files: &[(&str, &str)]) -> Sources {
    Sources {
        files: files
            .iter()
            .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
            .collect(),
        read: Vec::new(),
    }
}

fn run_in(screen: &str, files: &mut Sources) -> Analysis {
    let parsed = screen_compiler_tsx::parse(screen).unwrap();
    analyse_with(&parsed, &Options::new("screen.tsx"), files)
}

fn run(screen: &str, files: &[(&str, &str)]) -> Analysis {
    run_in(screen, &mut sources(files))
}

fn ok(screen: &str, files: &[(&str, &str)]) -> Module {
    let analysis = run(screen, files);
    match analysis.module {
        Some(module) => module,
        None => panic!(
            "expected a module, got {:#?}\n{screen}",
            analysis.diagnostics
        ),
    }
}

/// The outline of the screen's only screen.
fn ok_outline(screen: &str, files: &[(&str, &str)]) -> String {
    let module = ok(screen, files);
    assert_eq!(module.screens.len(), 1, "{screen}");
    outline(&module, &module.screens[0])
}

/// The outline of a screen written in one file.
fn flat(screen: &str) -> String {
    let parsed = screen_compiler_tsx::parse(screen).unwrap();
    let analysis = analyse(&parsed, &Options::new("screen.tsx"));
    let module = analysis.module.unwrap_or_else(|| {
        panic!("{:#?}", analysis.diagnostics);
    });
    outline(&module, &module.screens[0])
}

fn errors(screen: &str, files: &[(&str, &str)]) -> Vec<Diagnostic> {
    let analysis = run(screen, files);
    assert!(
        analysis.module.is_none(),
        "an error must stop conversion:\n{screen}"
    );
    analysis
        .diagnostics
        .into_iter()
        .filter(Diagnostic::is_error)
        .collect()
}

/// The one error the screen gives.
fn error(screen: &str, files: &[(&str, &str)]) -> Diagnostic {
    let all = errors(screen, files);
    assert_eq!(all.len(), 1, "{all:#?}\n{screen}");
    all.into_iter().next().unwrap()
}

const HEADER: &str = "\
import { Row, Text, Button } from \"nowayos:kit\";
export function Header({ title, onHome }) {
  return (
    <Row gap={8}>
      <Text role=\"heading\">{title}</Text>
      <Button onPress={onHome}>Home</Button>
    </Row>
  );
}
";

const FLAT: &str = "\
import { Button, Column, Row, Screen, Text } from \"nowayos:kit\";
export function S(props) {
  return (
    <Screen>
      <Column gap={12}>
        <Row gap={8}>
          <Text role=\"heading\">Check in</Text>
          <Button onPress={props.onHome}>Home</Button>
        </Row>
        <Text>Welcome</Text>
      </Column>
    </Screen>
  );
}
";

/// A screen of the kit's `<Column>` holding `body`, after `imports`.
fn screen(imports: &str, body: &str) -> String {
    format!(
        "{KIT}{imports}\nexport function S(props) {{ return <Screen><Column gap={{12}}>{body}</Column></Screen>; }}\n"
    )
}

#[test]
fn a_component_imported_from_a_file_is_the_screen_written_flat() {
    let body = "<Header title=\"Check in\" onHome={props.onHome} /><Text>Welcome</Text>";
    let expected = flat(FLAT);
    for (import, path) in [
        ("import { Header } from \"./header\";", "header.tsx"),
        ("import { Header } from \"./header.tsx\";", "header.tsx"),
        (
            "import { Header } from \"./parts/header\";",
            "parts/header.tsx",
        ),
    ] {
        let module = ok(&screen(import, body), &[(path, HEADER)]);
        assert_eq!(module.imported, vec![path.to_owned()]);
        assert_eq!(outline(&module, &module.screens[0]), expected, "{import}");
    }
    // Under another name, and as a default export.
    let aliased = screen(
        "import { Header as Top } from \"./header\";",
        "<Top title=\"Check in\" onHome={props.onHome} /><Text>Welcome</Text>",
    );
    assert_eq!(ok_outline(&aliased, &[("header.tsx", HEADER)]), expected);
    let default_export = HEADER.replace("export function", "export default function");
    let by_default = screen(
        "import Top from \"./header\";",
        &body.replace("Header", "Top"),
    );
    assert_eq!(
        ok_outline(&by_default, &[("header.tsx", &default_export)]),
        expected
    );
    // Exported by a list, under another name.
    let listed = format!(
        "{}\nexport {{ Header as Banner }};\n",
        HEADER.replace("export function", "function")
    );
    let by_list = screen(
        "import { Banner } from \"./header\";",
        &body.replace("Header", "Banner"),
    );
    assert_eq!(ok_outline(&by_list, &[("header.tsx", &listed)]), expected);
}

#[test]
fn an_imported_file_imports_from_its_own_folder_and_each_file_is_read_once() {
    let header = "\
import { Row } from \"nowayos:kit\";
import { Title } from \"./title\";
import { Home } from \"./home\";
export function Header({ title, onHome }) {
  return <Row gap={8}><Title text={title} /><Home onHome={onHome} /></Row>;
}
";
    let title = "\
import { Text } from \"nowayos:kit\";
export function Title({ text }) { return <Text role=\"heading\">{text}</Text>; }
";
    let home = "\
import { Button } from \"nowayos:kit\";
import { Title } from \"./title\";
export function Home({ onHome }) { return <Button onPress={onHome}>Home</Button>; }
";
    let mut files = sources(&[
        ("parts/header.tsx", header),
        ("parts/title.tsx", title),
        ("parts/home.tsx", home),
    ]);
    let analysis = run_in(
        &screen(
            "import { Header } from \"./parts/header\";",
            "<Header title=\"Check in\" onHome={props.onHome} /><Text>Welcome</Text>",
        ),
        &mut files,
    );
    let module = analysis.module.unwrap();
    assert_eq!(outline(&module, &module.screens[0]), flat(FLAT));
    assert_eq!(
        module.imported,
        ["parts/header.tsx", "parts/title.tsx", "parts/home.tsx"]
    );
    assert_eq!(
        files.read,
        ["parts/header.tsx", "parts/title.tsx", "parts/home.tsx"],
        "title.tsx is read once"
    );
}

#[test]
fn a_name_is_resolved_in_the_file_it_is_written_in() {
    // The part's file imports the kit's components it uses; the screen
    // need not, and the part may use a name the screen uses for another
    // thing.
    let card = "\
import { Column as Box, Text } from \"nowayos:kit\";
const GAP = 4;
function Label({ text }) { return <Text role=\"caption\">{text}</Text>; }
export function Card({ title }) { return <Box gap={GAP}><Label text={title} /></Box>; }
";
    let screen_file = "\
import { Column, Screen, Text } from \"nowayos:kit\";
import { Card } from \"./card\";
const GAP = 12;
function Label() { return <Text>Own label</Text>; }
export function S() {
  return <Screen><Column gap={GAP}><Label /><Card title=\"Fuel\" /></Column></Screen>;
}
";
    let flat_screen = "\
import { Column, Screen, Text } from \"nowayos:kit\";
const GAP = 12;
const CARD_GAP = 4;
export function S() {
  return (
    <Screen>
      <Column gap={GAP}>
        <Text>Own label</Text>
        <Column gap={CARD_GAP}><Text role=\"caption\">Fuel</Text></Column>
      </Column>
    </Screen>
  );
}
";
    // Two tokens called GAP cannot both be generated: the imported one
    // is refused, where it is declared.
    let clash = error(screen_file, &[("card.tsx", card)]);
    assert_eq!(clash.code, Code::DuplicateName, "{clash:#?}");
    assert_eq!(clash.loc.unwrap().file, 1);
    let renamed = card.replace("GAP", "CARD_GAP");
    let module = ok(screen_file, &[("card.tsx", &renamed)]);
    let expected = {
        let parsed = screen_compiler_tsx::parse(flat_screen).unwrap();
        let module = analyse(&parsed, &Options::new("screen.tsx"))
            .module
            .unwrap();
        outline(&module, &module.screens[0])
    };
    assert_eq!(outline(&module, &module.screens[0]), expected);

    // The screen's file does not see what the part's file imports.
    let unimported = "\
import { Column, Screen } from \"nowayos:kit\";
import { Card } from \"./card\";
export function S() { return <Screen><Column><Card title=\"Fuel\" /><Text>No</Text></Column></Screen>; }
";
    let e = error(unimported, &[("card.tsx", &renamed)]);
    assert_eq!(e.code, Code::UnknownElement, "{e:#?}");
    assert_eq!(e.loc.unwrap().file, 0);
    // Nor does the part's file see the screen's names.
    let leaning = "\
import { Column } from \"nowayos:kit\";
export function Card() { return <Column gap={GAP} />; }
";
    let all = errors(
        &screen(
            "import { Card } from \"./card\";\nconst GAP = 12;",
            "<Card />",
        ),
        &[("card.tsx", leaning)],
    );
    assert!(
        all.iter()
            .all(|e| e.loc.unwrap().file == 1 && e.message.contains("GAP")),
        "{all:#?}"
    );
}

#[test]
fn tokens_and_lists_may_be_imported_from_a_file() {
    let theme = "export const SPACE_MD = 16;\nexport const STOPS = [{ name: \"North\" }, { name: \"South\" }];\n";
    let module = ok(
        &screen(
            "import { SPACE_MD, STOPS } from \"./theme\";",
            "<Row gap={SPACE_MD}>{STOPS.map((stop) => <Text>{stop.name}</Text>)}</Row>",
        ),
        &[("theme.tsx", theme)],
    );
    let expected = flat(&format!(
        "{KIT}const SPACE_MD = 16;\nexport function S(props) {{ return <Screen><Column gap={{12}}><Row gap={{SPACE_MD}}><Text>North</Text><Text>South</Text></Row></Column></Screen>; }}\n"
    ));
    assert_eq!(outline(&module, &module.screens[0]), expected);
    assert_eq!(module.tokens.len(), 1);
    assert_eq!(module.tokens[0].loc.unwrap().file, 1);
}

#[test]
fn an_error_inside_an_imported_file_is_reported_there() {
    let card = "\
import { Column, Text } from \"nowayos:kit\";
export function Card() {
  return <Column><Text colour=\"red\">Fuel</Text></Column>;
}
";
    let analysis = run(
        &screen("import { Card } from \"./parts/card\";", "<Card />"),
        &[("parts/card.tsx", card)],
    );
    assert!(analysis.module.is_none());
    let e = analysis.diagnostics.iter().find(|d| d.is_error()).unwrap();
    assert_eq!(e.code, Code::UnknownProp, "{e:#?}");
    assert_eq!(e.loc, Some(Loc::new(3, 24).in_file(1)));
    let names = analysis.file_names("screens/screen.tsx");
    assert_eq!(names, ["screens/screen.tsx", "screens/parts/card.tsx"]);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    assert!(
        e.render_in(&names)
            .starts_with("screens/parts/card.tsx:3:24: error[unknown-prop]"),
        "{}",
        e.render_in(&names)
    );
    // A file that does not parse is reported there too, and read no more.
    let e = error(
        &screen("import { Card } from \"./card\";", "<Card />"),
        &[("card.tsx", "export function Card() { return <Column>; }\n")],
    );
    assert_eq!((e.code, e.loc.unwrap().file), (Code::Syntax, 1), "{e:#?}");
}

#[test]
fn a_file_that_cannot_be_read_is_reported_where_it_is_imported() {
    let at_import = |e: &Diagnostic| e.loc == Some(Loc::new(2, 1));
    let missing = error(&screen("import { Card } from \"./card\";", "<Card />"), &[]);
    assert_eq!(missing.code, Code::UnknownName);
    assert!(at_import(&missing), "{missing:#?}");
    assert!(missing.message.contains("card.tsx"), "{missing:#?}");
    for specifier in [
        "../card",
        "/card",
        "./card.js",
        "./data.json",
        "./a/../card",
    ] {
        let e = error(
            &screen(
                &format!("import {{ Card }} from \"{specifier}\";"),
                "<Card />",
            ),
            &[("card.tsx", HEADER)],
        );
        assert_eq!(e.code, Code::Unsupported, "{specifier}: {e:#?}");
        assert!(at_import(&e), "{specifier}: {e:#?}");
    }
    // Given as text alone, a screen reads no file.
    let parsed = screen_compiler_tsx::parse(&screen(
        "import { Header } from \"./header\";",
        "<Header title=\"a\" onHome={props.onHome} />",
    ))
    .unwrap();
    let analysis = analyse(&parsed, &Options::new("screen.tsx"));
    let e = analysis.diagnostics.iter().find(|d| d.is_error()).unwrap();
    assert_eq!(e.code, Code::Unsupported, "{e:#?}");
    assert!(e.message.contains("cannot be read"), "{e:#?}");
    // A type imported as a value is no component: refused where it is used.
    let types = "export type Header = { title: string };\n";
    let e = error(
        &screen("import { Header } from \"./header\";", "<Header />"),
        &[("header.tsx", types)],
    );
    assert_eq!(e.code, Code::UnknownElement, "{e:#?}");
    assert_eq!(e.loc, Some(Loc::new(3, 60)), "{e:#?}");
}

#[test]
fn what_an_import_names_must_be_exported_and_imported_by_name() {
    let e = error(
        &screen("import { Footer } from \"./header\";", "<Footer />"),
        &[("header.tsx", HEADER)],
    );
    assert_eq!(e.code, Code::UnknownName, "{e:#?}");
    assert!(e.message.contains("no export `Footer`"), "{e:#?}");
    let e = error(
        &screen("import Header from \"./header\";", "<Header />"),
        &[("header.tsx", HEADER)],
    );
    assert_eq!(e.message, "header.tsx has no default export", "{e:#?}");
    let all = errors(
        &screen("import * as Parts from \"./header\";", "<Text>a</Text>"),
        &[("header.tsx", HEADER)],
    );
    assert!(
        all.iter()
            .any(|e| e.code == Code::Unsupported && e.message.contains("by name")),
        "{all:#?}"
    );
    // Not exported at all: declared, but kept to its file.
    let private = HEADER.replace("export function", "function");
    let e = error(
        &screen("import { Header } from \"./header\";", "<Header />"),
        &[("header.tsx", &private)],
    );
    assert_eq!(e.code, Code::UnknownName, "{e:#?}");
}

#[test]
fn files_may_import_each_other_but_a_part_may_not_use_itself() {
    // header.tsx imports from the screen's file, which imports it.
    let header = "\
import { Row } from \"nowayos:kit\";
import { Title } from \"./screen\";
export function Header() { return <Row><Title /></Row>; }
";
    let screen_file = "\
import { Column, Screen, Text } from \"nowayos:kit\";
import { Header } from \"./header\";
export function Title() { return <Text>Hello</Text>; }
export function S() { return <Screen><Column><Header /></Column></Screen>; }
";
    let module = ok(screen_file, &[("header.tsx", header)]);
    assert_eq!(module.screens.len(), 1, "Title is a part, not a screen");
    // A part that uses itself through another file is refused.
    let looping = "\
import { Row } from \"nowayos:kit\";
import { Header as Again } from \"./header\";
export function Header() { return <Row><Again /></Row>; }
";
    let e = error(
        &screen("import { Header } from \"./header\";", "<Header />"),
        &[("header.tsx", looping)],
    );
    assert!(e.message.contains("uses itself"), "{e:#?}");
    // A name each file imports from the other is in a circle.
    let a = "import { X } from \"./b\";\nexport { X };\n";
    let b = "import { X } from \"./a\";\nexport { X };\n";
    let all = errors(
        &screen("import { X } from \"./a\";", "<X />"),
        &[("a.tsx", a), ("b.tsx", b)],
    );
    assert!(
        all.iter().any(|e| e.message.contains("in a circle")),
        "{all:#?}"
    );
}

#[test]
fn a_screen_reads_at_most_its_number_of_files() {
    // A chain of files, each importing the next.
    let mut files = Vec::new();
    for n in 1..=MAX_FILES {
        files.push((
            format!("f{n}.tsx"),
            format!("import {{ X }} from \"./f{}\";\nexport {{ X }};\n", n + 1),
        ));
    }
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(path, text)| (path.as_str(), text.as_str()))
        .collect();
    let all = errors(&screen("import { X } from \"./f1\";", "<X />"), &borrowed);
    assert!(
        all.iter()
            .any(|e| e.code == Code::Limit && e.message.contains(&MAX_FILES.to_string())),
        "{all:#?}"
    );
}

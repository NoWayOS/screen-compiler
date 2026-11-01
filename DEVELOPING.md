# Developing Screen Compiler

What Screen Compiler accepts, how to build and test it, and the contract it targets. What Screen Compiler is and why: [README.md](README.md).

It is **not** a general React compiler: it converts a declared subset exactly
and reports everything else with its line and column. It never approximates.

## Status

**Subset v0**, and the first parts of **subset v1**: TSX fixture → parse (`oxc_parser`) →
subset check → UI IR → deterministic Rust. The generated Rust targets a
**provisional contract** for NoWayOS's UI builder (v6), which `nowayos-ui`'s
`builder` module implements. `make contract-check`, part of `make ci`, builds every generated
screen against the real `nowayos-ui` from a sibling nowayos clone, for the
host and `aarch64-unknown-none`, builds each on a real tree, lays it out at
800×480 and draws it with NoWayOS's renderer; each fixture's frame must match
its approved image in `contract-check/golden/` pixel for pixel. See
[The target contract](#the-target-contract) and
[Approved images](#approved-images).

## Subset v0

| Accepted | Notes |
| --- | --- |
| `import { Screen, Column, … } from "nowayos:kit"` | the kit is imported by name (aliases allowed) from the virtual module `nowayos:kit` |
| `import React from "react"`, `import type …`, `type` / `interface` | ignored: no runtime content |
| `const SPACE_MD = 16;` | design token: a whole number of pixels, 0–65535 |
| `export function WelcomeScreen({ onStart }) { return <Screen>…</Screen>; }` | or a `const` arrow function, or `(props)` with `props.onStart`; the body is only the `return` |
| `<Screen>` | the root, exactly one child element |
| `<Column>`, `<Row>` | `gap`, `padding` (number or token), element children |
| `<Text>` | literal text only (JSX whitespace and `&amp;`-style references applied as React does) |
| `<Button onPress={onStart}>Label</Button>` | `onPress` names a callback prop, which becomes an action |
| `<TextInput id="email" placeholder="…" />` | `id` required |
| `<Spacer />`, `<Spacer size={N} />` | flexible or fixed |
| `id="…"` on any element | exposed as a handle in generated code |

## Subset v1, so far

| Accepted | Notes |
| --- | --- |
| `<Text>{props.name}</Text>`, or `<Text>{name}</Text>` with `name` destructured | text the application sets at run time: a **slot**, `b.slot(slots::NAME)`, never text compiled in. The prop must be the text's only content (`Name: {props.name}` is refused: write two `Text` elements); one slot per prop on a screen |
| `{props.ready && <X />}` | a **group** the application shows or hides, `b.group(groups::READY, x)`: hidden at first, and then taking no space |
| `{props.ready ? <X /> : <Y />}`, or `: null` | a group choosing between the two, `b.either(groups::READY, x, y)`: `<Y />` shows at first |
| `<TextInput id="phone" placeholder="…" maxLength={15} inputMode="tel" />` | `maxLength` 1–64 bytes, a number or a token (default 64); `inputMode` `text` (the default), `numeric` or `tel` (both digits only) or `email`. Every text input becomes `b.input(fields::PHONE, ui::Input::new()…)`, never v0's `text_input`; the runtime refuses a key outside its mode or beyond its length |
| `<Checkbox id="remind" label="…" />` | `b.checkbox(fields::REMIND, "…")`: a literal label of at most 256 bytes; `id` required |
| `<RadioGroup id="when" options={["…", "…"]} />` | `b.radio_group(fields::WHEN, &["…", "…"])`: 1 to 8 literal options, each at most 256 bytes (more needs the list picker, not built yet); `id` required |
| `<Button variant="secondary" onPress={…}>Back</Button>` | a button in a variant (contract v3): `variant` is `primary` (the default), `secondary` or `quiet`. `b.button_with("Back", actions::BACK, ui::ButtonVariant::Secondary)`; a primary button stays `b.button` |
| `<Card>…</Card>`, `<Notice tone="success">…</Notice>`, `<HeaderBar>…</HeaderBar>` | a column (Card, Notice) or a row (HeaderBar) drawn on a surface (contract v3), with `gap` and `padding` as for `Column` and `Row`; `tone` is `success`, `caution` or `warning`, and a notice needs one. `b.column_on(ui::Surface::Card, …)`, `b.row_on(ui::Surface::Header, …)` |
| `<Text role="title">…</Text>`, `<Text weight="bold">…</Text>` | text in a role of the theme's type scale (contract v2): `role` is `display`, `title`, `heading`, `body` (the default), `label` or `caption`, and `weight` `regular` or `bold`; display, title and heading are Bold only. `b.text_styled("…", ui::TextStyle::new(ui::TextRole::Title))`, or `b.slot_styled(…)` for a prop; body text in Regular stays `b.text` |
| `<Text lines={3}>…</Text>` | text that wraps onto up to N lines, 1 to 8 (contract v4): breaks after spaces at the node's width, which layout gives it first, then the height of its lines. `.lines(N)` on its `TextStyle`, through `b.text_styled` or `b.slot_styled`; more than 8, or anything but a whole number, is refused where written |
| `<ProgressBar value={props.fuelled} />` | a bar the application fills (contract v5): `b.progress(slots::FUELLED)`, its number shared with the slots'; the application sets how far it shows with `set_progress(slot, done, whole)`. `value` must be a prop, and a prop fills one slot or bar |
| `<Icon name="fuel" />` | one of Lucide's icons (ISC), drawn from a build-time alpha mask in the colour of text where it stands (contract v6): `b.icon(&ui::icons::FUEL)`, by its static, so a program links only the icons it names. `name` is a literal from `clipboard-list`, `fuel`, `house`, `message-square`, `shield-check` and `user-check`, NoWayOS's set (`make contract-check` holds the two lists the same) |

Every text input, checkbox and radio group is a field the application reads
and sets by number: each screen's `fields` module numbers them from 1 in
document order, named after their ids. A condition is one prop (`props.NAME`
or a destructured `NAME`), and what it shows is one element. Several conditions may test the same prop: their groups
share its number and show and hide together. Each screen's `groups` and
`slots` modules number them from 1 in document order, one per prop, as
`actions` does. Refused, each with its position: any other condition
(`!props.a`, `props.n > 0`, `a && b`, a call), `||` and `??`, `null` while
the condition holds, a condition inside a condition, text or a fragment as
what a condition shows, a condition as a screen's one child, text mixed with
a prop, the same prop as the text of two elements, two props whose names
give one group or slot name (`isReady` and `is_ready`, as for actions); a
character in any text that the glyph atlases of its role do not draw -
outside printable Latin-1, or, in the display role, anything but the digits,
the capitals, the space and `# % + , - . / : £` - named with the line and
column of its text; and, on the controls, a
`maxLength` outside 1–64 or not a number or token, any other `inputMode`,
options that are not 1 to 8 string literals, and state-bound props such as
`value`, `checked` and `onChange` (the application reads and sets each field
by its number).

Everything else — state and hooks, effects, lists (`.map`), CSS (`style`,
`className`, stylesheet imports), HTML elements, custom components,
fragments, spread props, inline handlers, computed text (`{a + b}`, a call, a
template), other imports (including `import { type … }`: write `import
type`), control or invisible characters in text, attribute strings that span
lines, and anything that is a syntax error in a module — is reported as an
error with its position, and nothing is written. `fixtures/unsupported/web-search.tsx`
shows most of them; `fixtures/unsupported/form-controls.tsx` shows what form
screens needed beyond subset v0 - conditional rendering, text from a prop, a
length limit and an input mode on a text input, a checkbox and a radio group,
which now convert - and text mixed with a prop, which stays refused.
`fixtures/parcel-status.tsx` converts with slots and groups, and
`fixtures/visit-booking.tsx` with every v1 form control built so far, and
`fixtures/queue-ticket.tsx` with the type scale's roles and Bold, and
`fixtures/check-in.tsx` with the button variants and the surfaces. `fixtures/unsupported/lost-property-*.tsx` are the three steps
of one synthetic form that together use every control a form-heavy kiosk
needs: a checkbox, a radio group, a list picker, a scrolling view, field
errors, several steps and an on-screen keyboard, besides text, buttons and
text inputs. All three are still refused; their errors are what the rest of
subset v1 must add.

## Layout

```text
screen-compiler/
├── crates/
│   ├── screen-compiler-ir/            the UI IR, names, diagnostics, validation
│   ├── screen-compiler-tsx/           TSX parsing (the only crate that uses oxc)
│   ├── screen-compiler-analyse/       subset checks and lowering to the IR
│   ├── screen-compiler-codegen-rust/  Rust generation + the target contract
│   └── screen-compiler-cli/           the `screen-compiler` command
├── contract-check/              every generated screen against the real nowayos-ui
│   │                            (a workspace of its own: Screen Compiler's crates never
│   │                            depend on NoWayOS)
│   └── golden/                  each fixture's approved image, FIXTURE/SCREEN.png
│                                (and SCREEN.shown.png, every group shown and slot filled)
└── fixtures/                    synthetic React screens only
```

## Prerequisites

* The NoWayOS dev container (Podman, or Docker with `ENGINE=docker`),
  built from a sibling clone of `nowayos` with `make container`. Every target
  runs inside it by default, so no host Rust is needed.
* Optional, for the host loop (`IN=host`): Rust **1.93.1**, pinned in
  `rust-toolchain.toml`. The Makefile never installs it: it sets
  `RUSTUP_AUTO_INSTALL=0`, so a missing toolchain is reported rather than
  downloaded. If `rustc +stable --version` prints 1.93.1, use
  `IN=host CARGO="cargo +stable"` (and `cargo +stable run …` for the
  commands below).

## Commands

Each target runs in the dev container (add `IN=host` to run the everyday
ones - all but `deny`, `contract-check`, `bless-images`, `reproducible` and
`ci` - on the host):

```sh
make test                      # unit, property, snapshot and CLI tests
make lint                      # rustfmt --check, then clippy with warnings denied
make doc                       # API docs, warnings as errors
make fixtures                  # run check and convert on every fixture
make bless                     # rewrite snapshots after a deliberate change, then review the diff
make deny                      # the NoWayOS baseline check, then licences, sources, bans and advisories (cargo-deny)
make sbom                      # a CycloneDX SBOM per workspace in build/sbom/ (NoWayOS's tools/sbom; container only)
make secrets                   # the tree and every commit scanned for keys and credentials, offline (NoWayOS's tools/secrets-scan; secrets-allow.toml; container only)
make contract-check            # every generated screen against the real nowayos-ui (../nowayos), host and aarch64-unknown-none, drawn; each fixture's frame matches its approved image
make bless-images              # redraw the approved images after a deliberate change, then review them by eye
make reproducible CARGO="cargo +stable"   # from the host: byte-identical Rust on the host and in the container, drawn exactly as approved
make no-nowayos                # Screen Compiler's own crates depend on no NoWayOS crate (cargo metadata)
make ci                        # the secrets scan, lint, test, doc, contract-check, no-nowayos, cargo-deny and the SBOMs
```

To run the tool itself, give `make run` its command line (in the container,
or on the host with `IN=host`):

```sh
make run ARGS="check fixtures/welcome.tsx"
make run ARGS="convert fixtures/welcome.tsx" > welcome.rs   # the Rust, written to a file
```

`screen-compiler convert IN.tsx [-o OUT.rs]` writes Rust to `OUT.rs` or standard
output; `screen-compiler check IN.tsx` only reports. Exit status: 0 success, 1 not
convertible (diagnostics on standard error, nothing written), 2 usage or I/O
error. `OUT.rs` is replaced whole: it is either left as it was or holds the
complete new file. Diagnostics read `FILE:LINE:COLUMN: error[code]: message`,
with control and invisible characters from the source or file name written
as `\u{…}`.

Inputs are limited to 64 KiB. Screen Compiler is meant for screens a team writes,
not for untrusted files: some pathological nesting (for example thousands of
nested `a<`) makes the parser's time and memory grow with the square of the
input.

## The target contract

Generated code imports `nowayos_ui::builder` as `ui` and uses the `UiBuilder`
trait (children-first, fallible node construction), `Layout`, `Input` and
`InputMode`, and the numbers `ActionId`, `FieldId`, `GroupId` and `SlotId`
and, for text in a role, `TextStyle` and `TextRole`, and for the look of
buttons and surfaces, `ButtonVariant`, `Surface` and `Tone` (contract v3),
text wrapped onto several lines, `TextStyle::lines` (contract v4), and
a progress bar, `progress` (contract v5), and icons, `icon` with the
`icons` statics (contract v6;
earlier versions' calls are unchanged, so screens generated for them still
build). Their reference definitions are compiled in
`crates/screen-compiler-codegen-rust/src/contract.rs`, and the test suite compiles
every snapshot of generated code against them and checks that running it
builds exactly the tree the IR describes. Each screen becomes a module with a
`build` function, an `actions` module (numbered from 1 in document order),
`fields`, `groups` and `slots` modules when it has them (likewise: one per
value-holding control, named after its id; one per prop) and a `Nodes` struct
of handles for elements with an `id`.

Generated files are pure ASCII, carry an `@generated` header, and are
byte-for-byte reproducible. Run `screen-compiler convert` (in the dev loop or in CI)
and check the output in as a module file declared
`#[rustfmt::skip] mod welcome;`, so that `cargo fmt` leaves it exactly as
generated. Give that module a name other than the screen module inside it:
`details-form.tsx` generates `pub mod details_form`, so declaring the file as
`mod details_form;` trips clippy's `module_inception` lint (an error under
`-D warnings`); `mod details_form_screen;` does not. Running Screen Compiler from
a product's build script instead, with its crates as build-dependencies,
would bring oxc and the licence exceptions below into that product's
dependency graph, resolved without this repository's `Cargo.lock`.

## Approved images

`make contract-check` converts every fixture afresh, builds each screen on a
real `nowayos-ui` tree, lays it out at 800×480 (the Raspberry Pi Touch
Display's resolution) in the light theme - its text 24 px, from NoWayOS's
glyph atlases - and draws it with NoWayOS's renderer (`nowayos-render-eg`)
into an in-memory frame. The
frame must match the screen's approved image,
`contract-check/golden/FIXTURE/SCREEN.png`, pixel for pixel. A screen with
groups or slots is drawn a second time as the application would show it with
every group visible and each slot holding "Text from the application, N", and
must match `SCREEN.shown.png` too. A frame that
differs is written beside the image as `SCREEN.actual.png` (never committed)
so the two can be compared, and an image no converted screen draws fails the
check too.

The images change only on purpose. After a deliberate change to a fixture,
to Screen Compiler or to NoWayOS's layout or drawing, run `make bless-images`
(always in the dev container, where approved images are drawn; it prints the
NoWayOS commit that drew them), look at every image it changed - PNG, which
GitHub and any image viewer show - and commit them with the
change, naming that NoWayOS commit in the message. A person approves every
new or changed image. `make reproducible CARGO="cargo
+stable"`, run from the host where the host's Rust is 1.93.1, shows that each
fixture converts to byte-identical Rust on the host and in the dev container
and draws on the host exactly the approved images `make ci` draws in the
container. It is not part of `make ci`, which runs only in the container.

## Licence

**MIT** — see [`LICENSE`](LICENSE). Every crate of the tool declares `license = "MIT"` and
stays `publish = false`, and every source file but the snapshots carries an
`SPDX-License-Identifier: MIT` line. `contract-check/` is a CI-only harness
that builds generated screens against NoWayOS's crates from the sibling
clone; it declares `license = "MIT"` too, and `make deny` checks every crate
in its graph. NoWayOS's crates there are MIT OR Apache-2.0, but for one
exception: NoWayOS's glyph atlases, `nowayos-glyphs`, which `nowayos-ui`
measures text with and `nowayos-render-eg` draws text from, under their
font's licence, OFL-1.1. It copies no NoWayOS code, is never published, and
nothing it builds is distributed, so it carries no font notice. Its tests
also use `embedded-graphics` (MIT OR Apache-2.0) and its dependencies,
through NoWayOS's renderer, checked by `make deny`. The approved images in
`contract-check/golden/` show synthetic screens only, their text drawn from
those atlases: images made with the font, which the OFL does not cover, so
they carry no notice either. Treat everything here as public from the first
commit.

**Generated code is yours.** The Rust that Screen Compiler generates from your
screens belongs to whoever runs the tool, to license as they wish: Screen Compiler's
MIT licence covers Screen Compiler itself, not its output.

Dependencies outside the house licence list are recorded in `deny.toml`:
`unicode-id-start` and `unicode-ident` (Unicode-3.0) and `dragonbox_ecma`
(Apache-2.0 WITH LLVM-exception), signed off by the project owner on
2026-10-01. All three are used by the build-time tool only and never
ship on a device. A distributed Screen Compiler binary will need a third-party
notices file, including the Unicode-3.0 notice.

## Further reading

* The guides, among them making screens from React: [www.nowayos.com](https://www.nowayos.com/docs).
* NoWayOS's UI core, which the generated Rust targets: `nowayos-ui` and its
  documentation in [NoWayOS](https://github.com/NoWayOS/nowayos).

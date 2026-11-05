# Developing Screen Compiler

What Screen Compiler accepts, how to build and test it, and the contract it targets. What Screen Compiler is and why: [README.md](README.md).

It is **not** a general React compiler: it converts a declared subset exactly
and reports everything else with its line and column. It never approximates.
The subset grows, a construct at a time, towards the React and CSS that real
interfaces use; what needs JavaScript or a browser on the device - state,
hooks and effects, raw colours, animation - stays out.

## Status

**Subset v0** and **subset v1**: TSX fixture → parse (`oxc_parser`) →
subset check → UI IR → deterministic Rust. The generated Rust targets a
**provisional contract** for NoWayOS's UI builder (v12), which `nowayos-ui`'s
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
| `<Button onPress={onStart}>Label</Button>` | `onPress` names a callback prop, which becomes an action. React DOM's `onClick` is the same, and a button takes one of the two, never both. Either may be a function that only calls a callback prop with no arguments - `() => props.onStart()`, `() => { onStart(); }` - which is that action; a function that does anything more (two calls, an argument, a parameter, `async`, `?.`) is refused where it starts |
| `<TextInput id="email" placeholder="…" />` | `id` required |
| `<Spacer />`, `<Spacer size={N} />` | flexible or fixed |
| `id="…"` on any element | exposed as a handle in generated code |

## Subset v1

| Accepted | Notes |
| --- | --- |
| `<Text>{props.name}</Text>`, or `<Text>{name}</Text>` with `name` destructured | text the application sets at run time: a **slot**, `b.slot(slots::NAME)`, never text compiled in; one slot per prop on a screen |
| `<Text>Hello, {props.name}!</Text>`, `<Text>{props.count} items</Text>`, or `` <Text>{`Hello, ${props.name}!`}</Text> `` | a slot **between literal text** compiled in (contract v14): `b.slot_template(slots::NAME, &ui::Template { before: "Hello, ", after: "!" })`, or `b.slot_template_styled(…, style)` in a role. The literals show from the first frame, the application sets only the prop's part (at most 64 bytes, as any slot), and the whole is measured, wrapped and aligned as one text. JSX text, string expressions and a template literal's text may mix; the literals take at most 64 bytes together and are checked as any text in the role. In a component of the file, a literal given for the prop joins the text, and an omitted one is nothing in JSX and `undefined` in a template literal, as React shows them. Refused where written: a second prop in one text (write a `<Text>` for each, or join them in the application), and any other expression (`{a + b}`, a call). `fixtures/table-ready.tsx` has each form |
| `{props.ready && <X />}` | a **group** the application shows or hides, `b.group(groups::READY, x)`: hidden at first, and then taking no space |
| `{props.ready ? <X /> : <Y />}`, or `: null` | a group choosing between the two, `b.either(groups::READY, x, y)`: `<Y />` shows at first |
| `{!props.ready && <X />}`, or `{props.ready ? null : <X />}` | the prop's group negated (contract v13), `b.unless(groups::READY, x)`: shown at first, and while the application hides the group; it shares the prop's group number. `{!props.ready ? <X /> : <Y />}` is `{props.ready ? <Y /> : <X />}` |
| `{props.step === "review" && <X />}`, `{props.step === "pay" ? <X /> : <Y />}` | a **switch** the application sets to one case (contract v13): one per prop, each string literal it is compared with one case, `b.case(switches::STEP, switches::step::REVIEW, x)` or `b.case_or(…, x, y)`. The screen's `switches` module numbers switches from 1 in document order, each beside a module of its cases, `u8`s from 0 in the order first written; on no case at first, and what a case does not show takes no space. Refused where written: comparing two props, a prop with a number or a computed value, or two literals; `!==`, `==` and the other comparisons; `!!`; `null` while a case holds; a literal that cannot name a constant, or two that name the same one (`"review"` and `"Review"`); more than 256 cases; a prop whose cases' module would be a Rust keyword or `ui`. Each case is drawn on its own (`SCREEN.case-SWITCH-CASE.png`); `fixtures/bike-hire.tsx` has a switch and a negation |
| `<TextArea id="comments" rows={5} maxLength={1000} placeholder="…" />` | several lines a person types (contract v19): `b.text_area(fields::COMMENTS, 5, 1000, "…")`, numbered with the fields and read as a text input is. Wrapped at its width, Enter starting a line, scrolled to keep the caret in view. `id` and `maxLength` (1 to 65,535 bytes, a number or a design token) required: the bytes come out of the text store of the `Tree<N, T>` the screen is built into. `rows` 1 to 32 (default 2, as HTML's `<textarea>`) |
| `<TextInput id="phone" placeholder="…" maxLength={15} inputMode="tel" />` | `maxLength` 1–64 bytes, a number or a token (default 64); `inputMode` `text` (the default), `numeric` or `tel` (both digits only) or `email`. Every text input becomes `b.input(fields::PHONE, ui::Input::new()…)`, never v0's `text_input`; the runtime refuses a key outside its mode or beyond its length |
| `<Checkbox id="remind" label="…" />` | `b.checkbox(fields::REMIND, "…")`: a literal label of at most 256 bytes; `id` required |
| `<RadioGroup id="when" options={["…", "…"]} />` | `b.radio_group(fields::WHEN, &["…", "…"])`: 1 to 8 literal options, each at most 256 bytes (more is refused, with help naming `<Select>`); `id` required |
| `<Button variant="secondary" onPress={…}>Back</Button>` | a button in a variant (contract v3): `variant` is `primary` (the default), `secondary` or `quiet`. `b.button_with("Back", actions::BACK, ui::ButtonVariant::Secondary)`; a primary button stays `b.button` |
| `<Card>…</Card>`, `<Notice tone="success">…</Notice>`, `<HeaderBar>…</HeaderBar>` | a column (Card, Notice) or a row (HeaderBar) drawn on a surface (contract v3), with `gap` and `padding` as for `Column` and `Row`; `tone` is `success`, `caution` or `warning`, and a notice needs one. `b.column_on(ui::Surface::Card, …)`, `b.row_on(ui::Surface::Header, …)` |
| `<Text role="title">…</Text>`, `<Text weight="bold">…</Text>` | text in a role of the theme's type scale (contract v2): `role` is `display`, `title`, `heading`, `body` (the default), `label` or `caption`, and `weight` `regular` or `bold`; display, title and heading are Bold only. `b.text_styled("…", ui::TextStyle::new(ui::TextRole::Title))`, or `b.slot_styled(…)` for a prop; body text in Regular stays `b.text` |
| `<Text lines={3}>…</Text>` | text that wraps onto up to N lines, 1 to 8 (contract v4): breaks after spaces at the node's width, which layout gives it first, then the height of its lines. `.lines(N)` on its `TextStyle`, through `b.text_styled` or `b.slot_styled`; more than 8, or anything but a whole number, is refused where written |
| `<Text>Press <strong>Save</strong>, or read <a onClick={props.onGuide}>the guide</a>.</Text>` | a rich text (contract v21): `b.rich_text({ const RUNS: &[ui::Run] = &[ui::Run::new("Press "), ui::Run::new("Save").bold(), …]; RUNS }, style)`, one paragraph of runs wrapped as one text. Inside a `<Text>`, around literal words (and `{" "}`): `<strong>`/`<b>` Bold, `<em>`/`<i>` italics (drawn slanted until an italic face is signed off), `<code>` code (on a tint), `<a onClick={props.onX}>` (or `onPress`) a link - a tab stop raising the action, numbered with the buttons' - and `<span style={{ color: "error" }}>` a theme colour; they nest. Refused where written: any other element, a prop's text, `href` (a station never leaves for an address), a span's style naming more than its colour, and `lines`, `align` or the display role on the text |
| `<Text maxLength={10240}>{props.article}</Text>` | paragraphs the application sets at run time, up to N bytes (1 to 65,535, a number or a design token; contract v16): `b.long_text(slots::ARTICLE, 10240, style)`, numbered with the slots and set as one. Wrapped onto as many lines as they need, a line break starting a new paragraph; the bytes live in the text store of the `Tree<N, T>` the screen is built into, so the tree must hold `T` bytes for them. One prop alone: literal text, `lines` or the display role is refused where written |
| `<ProgressBar value={props.fuelled} />` | a bar the application fills (contract v5): `b.progress(slots::FUELLED)`, its number shared with the slots'; the application sets how far it shows with `set_progress(slot, done, whole)`. `value` must be a prop, and a prop fills one slot or bar |
| `<Icon name="fuel" />` | one of Lucide's icons (ISC), drawn from a build-time alpha mask in the colour of text where it stands (contract v6): `b.icon(&ui::icons::FUEL)`, by its static, so a program links only the icons it names. `name` is a literal from `clipboard-list`, `fuel`, `house`, `message-square`, `shield-check` and `user-check`, NoWayOS's set (`make contract-check` holds the two lists the same) |
| `<Image src="harbour" alt="…" fit="cover" style={{ width: 320, height: 200 }} />` | one of the product's pictures, converted at build time by NoWayOS's picture tool (contract v15): `b.image(&crate::pictures::HARBOUR, ui::Fit::Cover, "…", ui::Layout::new().width(320).height(200))`, by its static in the `pictures` module at the root of the crate the screens are compiled in, so a program links only the pictures it names. `src` must be a name the `--pictures FILE` list holds (the `names` file the tool writes beside the pictures); `alt` is required (`""` for decoration); `fit` - or the style's `objectFit` - is `fill` (the default), `contain` or `cover`; without a width or height the picture takes its own size. The fixtures name NoWayOS's samples (`fixtures/pictures.names`) |
| `<Text align="center">…</Text>` | text aligned in its node (contract v7): `align` a literal `start` (the default), `center` or `end`, each wrapped line on its own; `.align(ui::TextAlign::Center)` on its `TextStyle`. Refused on anything but `<Text>` |
| `import { space } from "nowayos:kit"`; `gap={space.md}`, `padding={space.lg}`, `<Spacer size={space.sm} />` | the theme's spacing tokens (contract v7), `xs` to `xl`, measured by the theme when the screen is laid out: `ui::Layout::new().gap_space(ui::Space::Md)`, `.padding_space(…)`, `b.spacer_space(ui::Space::Sm)`. A number or a screen's own `const` stays fixed; an unknown token, or a token as `maxLength`, is refused. A screen naming a token is also drawn in the large-text theme (`SCREEN.large.png`) |
| `style={{ … }}` | a declared subset of CSS (contract v8), an object literal of static properties: on `<Column>`, `<Row>`, `<Card>`, `<Notice>` and `<HeaderBar>`, `gap`, `padding`, `width`, `height`, `minWidth`, `maxWidth`, `alignItems`, `justifyContent`, `backgroundColor`, `borderColor`, `borderWidth` (0-16) and `borderRadius` (0-64); on `<Text>`, `color` and `textAlign`; on `<Button>`, `backgroundColor` and `color`. Sizes in layout units (`360`, or CSS's `"360px"`, as `gap` and `padding` also take as props) or a token; colours by the theme's name (`background`, `text`, `text-muted`, `placeholder`, `button`, `button-text`, `input`, `input-border`, `focus`, `error`), or as CSS's `var(--text-muted)`, never a value, so the large-text theme recolours the screen; `alignItems` and `justifyContent` take `flex-start`/`start`, `center`, `flex-end`/`end`, `stretch` (and `space-between`), `textAlign` `left`/`start`, `center`, `right`/`end`. `ui::Layout::new().width(N)…align(ui::Align::Center).justify(ui::Justify::SpaceBetween).look(ui::Look::NONE.fill(ui::Colour::Input)…)`, `.colour(…)` on a `TextStyle`, `b.button_look(…)`. Refused where written: a property given both as a prop and in the style, any other property, a value from a prop or computed, a spread or computed key, any CSS unit but px (`rem`, `em`, `%`, `vh`, `vw`…), and `var(--name)` with a fallback or naming no theme colour. `fixtures/parking-paid-css.tsx` is `parking-paid.tsx` written that way, and draws exactly as it does. A screen naming a colour is drawn in the large-text theme too, and every text checked at 4.5:1 in both themes |
| `style={{ boxShadow: "medium", background: "linear-gradient(to right, input, focus)", opacity: 0.6 }}` on a container | a theme's shadow, a gradient and an opacity (contract v22): `.look(ui::Look::NONE.shadow(ui::Elevation::Medium).gradient(ui::Gradient::linear(90, ui::Colour::Input, ui::Colour::Focus)).opacity(153))`. `boxShadow` names `low`, `medium` or `high`, the theme's, never a CSS shadow; `background` is `linear-gradient(…)` - an angle in degrees or `to top`, `to right`, `to bottom` or `to left` first, if any (180 when not) - or `radial-gradient(…)`, of two of the theme's colours by name or `var(--name)` (a single colour is `backgroundColor`); `opacity` a number from 0 to 1, over the container and everything in it. NoWayOS composes them in bounded tiles (ADR-0027); text over a gradient is checked for contrast at its worse end |
| `<ScrollView id="answers"><Column>…</Column></ScrollView>` | a clipped column that scrolls down (contract v9): `b.scroll_view(ui::Layout::new(), column)`. Exactly one child element. A screen may hold any number, one inside another (contract v17). Also drawn scrolled to its end (`SCREEN.scrolled.png`) |
| `<Column style={{ overflowY: "auto", height: 240 }}>…</Column>` | a container that scrolls (contract v17): `overflowY` down, `overflowX` across, `overflow` - or both - both ways, each `auto` or `scroll`. It becomes a scroll view of the container's size holding the container with the rest of its style: `{ let inner = b.column(layout, &[…])?; b.scroll_view_axes(ui::Layout::new().height(240), ui::ScrollAxes::Vertical, inner) }`. `hidden` and `visible` are refused where written. NoWayOS shows a scrollbar along each axis its content overflows, which can be dragged |
| `<VirtualList rowHeight={48}>{props.items.slice(0, 12).map((item) => <Row>…</Row>)}</VirtualList>` | a virtual list (contract v18): `b.virtual_list(ui::Layout::new(), lists::ITEMS, 48, &[row1, …, row12])`, with `pub const ITEMS: ui::ListId` in the screen's `lists` module. The sliced list's rows - their slots numbered as any list's rows are, `ITEMS_1_NAME` … - are a pool built once: the application sets the list's count (`Tree::set_list_count`, as many items as it likes) and binds each row NoWayOS asks it to (`UiEvent::ListMoved`, `Tree::list_row`) as the list scrolls. `rowHeight` is 8 to 512 units, a number or a design token; anything inside but one list from a prop is refused where written |
| `<Select id="kind" placeholder="…" options={[…]} rows={5} />` | a list picker (contract v10): `b.list_picker(fields::KIND, &[…], ui::Picker::new().placeholder("…").rows(5))`. `id` required; 1 to 64 literal options, each checked as other text; `rows` a whole number 3 to 8 (default 5) |
| `<Dialog open={props.confirming}><Column>…</Column></Dialog>`; `<Button id="more" …/><Menu for="more" open={props.menuOpen}><Column>…</Column></Menu>`, or `<Popover …>`; `<Tooltip for="delete">Deletes it for good</Tooltip>` | layers over the screen (contract v20): `b.dialog(layers::CONFIRMING, column)`, `b.menu(layers::MENU_OPEN, n4, column)`, `b.popover(…)`, `b.tooltip(n12, "…")`, with `pub const CONFIRMING: ui::LayerId` in the screen's `layers` module, one per prop. The application opens and closes each by its layer (`Tree::open`, `Tree::close`) while its prop is true; a dialog centres over the screen and veils it, a menu or popover shows beside the control `for` names and closes itself on Escape or a press outside (`UiEvent::Closed`), and a tooltip shows while the pointer rests on its control or it has focus after a key. A dialog, menu or popover holds exactly one child element; `for` names an element written before it, never one around it; one prop opens one layer. Refused where written otherwise. `make contract-check` also draws each layer open (`SCREEN.layer-NAME.png`) |
| `<Keyboard />` | the on-screen keyboard (contract v11): `b.keyboard()`. No children or props but `id`; a second in a screen, or one in a screen with no `<TextInput>`, is refused where written. Also drawn once for each mode its screen's inputs take (`SCREEN.keys-digits.png` and the others) |
| `<>…</>`; `function Bar({ title, onHome }) { return <HeaderBar>…</HeaderBar>; }` used as `<Bar title="…" onHome={props.onHome} />` | **fragments and the screen's own components** (*parts*), expanded at build time - no contract change. A fragment's children take its place wherever children are taken (a fragment of one element is that element where a condition shows one). A *part* is a function component that the screen's files use as an element - in the screen's file (every other component there is a screen), or imported from a `.tsx` file in its folder or below by a path starting `./` (`import { Bar } from "./parts/bar"`, named or default; such a file may import from its own folder in turn): each use is replaced by what it returns, each of its props by the value written where it is used - a literal, a design token, one of the screen's props passed through - and `{children}` by the children written between its tags; parts may use parts and pass props and children on, and a screen may be a part at its root (a shared frame). As in React, a prop the use does not give is `undefined`: an attribute set from it is left out, a child or condition made of it shows nothing. The screen is then checked and lowered as if written flat, so actions, groups, slots and fields keep one number each per screen, and an error is reported where it is written. Refused where written: a part that uses itself, directly or through others; a spread on a part; the whole props object passed on; `children` as a prop's value; a name a part uses that is neither its prop nor declared in the file; parts more than 16 deep, or a screen expanding past eight times the walk's node limit. **From other files**: each file is read once, a screen and the files it reads 32 at most, each under the 64 KiB cap; design tokens and `const` lists may be imported too. Every name is resolved in the file it is written in, as in JavaScript - a part's file imports the kit components it uses, and sees none of the screen's names - and an error inside an imported file is reported there (`parts/bar.tsx:3:5: error[…]`), as are the generated code's comments. Refused where the import is written: a path that leaves the folder (`../`, an absolute path, a symbolic link leading out), a file that is not `.tsx`, a missing or unreadable file, a namespace import (`import * as`), a name the file does not export, a name each file imports from the other. A file of parts checked alone is no screen; the coverage report counts it converting when a screen that converts imports it. `fixtures/check-in-parts.tsx` is `check-in.tsx` built of parts, and `fixtures/check-in-files.tsx` the same built of parts from `fixtures/parts/`; both must draw its approved image |
| `{DRINKS.map((drink) => <Text key={drink}>{drink}</Text>)}`; `{props.lines.slice(0, 4).map((line) => <Row>…{line.name}…</Row>)}` | **lists** - no contract change. `.map` over an array literal or a top-level `const` array (or one of these `.slice(0, N)`) is expanded at build time: one copy of what the callback returns for each item, its parameter given the item - an object item's fields as a part's props are given, the item itself to `(item) => …`; `key` is dropped. A screen's prop sliced, `props.items.slice(0, N).map((item) => <Row>…</Row>)`, `N` 1 to 32, gives `N` rows the application shows and fills: row `r` is the group `items_r` (hidden at first), each field `item.name` the slot `items_r_name`, the item itself the slot `items_r_item`; a callback may also read the screen's props, as a closure does. Refused where written: a list from a prop with no most (`.slice(0, N)`), a most that is not a whole number 1 to 32, a row of more than one element, a list from a prop inside a row of another, a callback taking more than the item or doing more than return, and listing anything else. `fixtures/order-summary.tsx` has both |
| `<FieldError for="phone" />` | the line where a field's error shows (contract v12): `b.field_error(fields::PHONE)`. `for` names the `id` of a control holding a field on the screen, before or after it; one per field. A line high even while empty; the application sets the errors. Each field's error is set in the shown frame |

Every text input, checkbox and radio group is a field the application reads
and sets by number: each screen's `fields` module numbers them from 1 in
document order, named after their ids. A condition is one prop (`props.NAME`
or a destructured `NAME`), and what it shows is one element. Several conditions may test the same prop: their groups
share its number and show and hide together. Each screen's `groups` and
`slots` modules number them from 1 in document order, one per prop, as
`actions` does. Refused, each with its position: any other condition
(`props.n > 0`, `a && b`, a call; `!` a prop and `===` a string literal are
contract v13's, above), `||` and `??`, a condition inside a condition, text or a fragment of
several elements as what a condition shows, a condition as a screen's one child, a second prop
in one text, the same prop as the text of two elements, two props whose names
give one group or slot name (`isReady` and `is_ready`, as for actions); a
character in any text that the glyph atlases of its role do not draw -
outside printable Latin-1, or, in the display role, anything but the digits,
the capitals, the space and `# % + , - . / : £` - named with the line and
column of its text; and, on the controls, a
`maxLength` outside 1–64 or not a number or token, any other `inputMode`,
options that are not 1 to 8 string literals, and state-bound props such as
`value`, `checked` and `onChange` (the application reads and sets each field
by its number).

Everything else — state and hooks, effects, lists other than the above, CSS outside the
declared `style` subset (`className`, stylesheet imports, any other
property or value), HTML elements, components imported from packages or from outside the
screen's folder, spread props, inline handlers that do more than call a
callback prop, computed text (`{a + b}`, a call,
a second prop in one text), other imports (including `import { type … }`: write `import
type`), control or invisible characters in text, attribute strings that span
lines, and anything that is a syntax error in a module — is reported as an
error with its position, and nothing is written. `fixtures/unsupported/web-search.tsx`
shows most of them. `fixtures/form-controls.tsx` shows what form screens
needed beyond subset v0 - conditional rendering, text from a prop, a length
limit and an input mode on a text input, a checkbox and a radio group - and
converts since text could mix literals and a prop (contract v14).
`fixtures/parcel-status.tsx` converts with slots and groups, and
`fixtures/visit-booking.tsx` with every v1 form control built so far, and
`fixtures/queue-ticket.tsx` with the type scale's roles and Bold, and
`fixtures/check-in.tsx` with the button variants and the surfaces. `fixtures/lost-property-*.tsx` are the three steps
of one synthetic form that together use every control a form-heavy kiosk
needs: a checkbox, a radio group, a list picker, a scrolling view, field
errors and an on-screen keyboard, besides text, buttons and text inputs;
several steps are the application switching screens. All three convert.

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
│                                (and SCREEN.shown.png, every group shown and slot filled;
│                                SCREEN.case-SWITCH-CASE.png, each case of each switch)
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
make run ARGS="audit fixtures"                               # how much of a folder converts
make run ARGS="subset"                                       # the subset as data (JSON)
make run ARGS="subset --dts"                                 # the same as TypeScript declarations of the kit
```

`screen-compiler subset` prints the subset as data, from the tables the
analyser checks against (`crates/screen-compiler-analyse/src/kit.rs`): every
component of the kit, what it holds between its tags, the props and style
properties it takes, which props it must be given, and the values each
accepts - its keywords, its range, whether a design token or a spacing token
may stand for it - and the theme's colours and spacing tokens.
`kit/subset.json` is that output, committed; `make test` fails if it differs
(`make bless` rewrites it), and fails if the analyser disagrees with the
tables: each component must convert with its required props and not without
any of them, every value listed must convert, and values outside the lists,
and props and properties the tables leave out, must be refused. The
website's list of what converts is generated from a copy of
`kit/subset.json`; this file's tables follow it, and a change to the kit's
tables changes them in the same commit.

`screen-compiler subset --dts` writes the same tables as TypeScript
declarations of `nowayos:kit`, committed as `kit/nowayos-kit.d.ts` and held
to the tables by `make test` like `kit/subset.json`. Your editor then knows
the kit: an editor that bundles TypeScript, such as VS Code, flags an unknown
component or prop, an HTML element, a missing required prop, a keyword, colour
or number outside the lists, or text where an element must go, as you type -
no Node.js or TypeScript to install. Put the file beside your screens with a
`tsconfig.json` like `fixtures/tsconfig.json` (`"jsx": "preserve"`, no React
types: the declarations bring their own `JSX`). Screen Compiler still has the
last word: a text's characters, an id's form, where a field error or the
keyboard may go and the other rules types cannot say are checked when it
converts.

`screen-compiler audit DIR` is the coverage report: it counts every construct
written in the `.tsx` files under `DIR` - elements, props, style properties,
hooks, lists, conditions, imports - as in the subset, on the road towards
React and CSS (naming the step that would convert it), out by design, or
unknown, and says which files convert. A construct it does not know is
unknown, never counted as converting, and a file converts only if `check`
passes it. It names constructs and quotes no code. Its report on `fixtures/`
is approved in `crates/screen-compiler-analyse/tests/snapshots/audit-fixtures.txt`
(`make bless`). Run it only on code you trust: the parser's limits are
`check`'s.

`screen-compiler convert IN.tsx [-o OUT.rs]` writes Rust to `OUT.rs` or standard
output; `screen-compiler check IN.tsx` only reports. Exit status: 0 success, 1 not
convertible (diagnostics on standard error, nothing written), 2 usage or I/O
error (`audit` exits 0 whatever it finds). `OUT.rs` is replaced whole: it is either left as it was or holds the
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
a progress bar, `progress` (contract v5), icons, `icon` with the
`icons` statics (contract v6), text alignment, `TextAlign`, and the
theme's spacing tokens, `Space` (contract v7), the declared style subset's
`Look`, `Colour`, `Align`, `Justify` and `button_look` (contract v8), a
scroll view, `scroll_view` (contract v9), a list picker, `list_picker` and
`Picker` (contract v10), the on-screen keyboard, `keyboard` (contract v11),
and a field's error line, `field_error` (contract v12;
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
must match `SCREEN.shown.png` too; a screen with switches is drawn again
for each case of each, that switch set to it, and must match
`SCREEN.case-SWITCH-CASE.png`. A frame that
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

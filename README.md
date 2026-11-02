# NoWayOS Screen Compiler

**Design in React. Ship native Rust. No browser on the device.**

> Part of NoWayOS: no way to exploit what was never built in.

The Screen Compiler converts React screens, written in TSX to a declared subset, into
Rust for the UI runtime of [NoWayOS](https://github.com/NoWayOS/nowayos),
NoWayOS's Single Purpose OS. It runs at build time, on your
machine or in CI, and never ships. Your designers and front-end developers
keep the tools and skills they have; the device gets plain generated Rust,
with no JavaScript engine, no browser and no web page to attack.

## From this

```tsx
export function WelcomeScreen({ onStart, onHelp }: WelcomeProps) {
  return (
    <Screen>
      <Column gap={SPACE_SM} padding={SPACE_LG}>
        <Text id="title">Welcome</Text>
        <Text>Touch Start to begin.</Text>
        <Spacer />
        <Button id="start" onPress={onStart}>Start</Button>
        <Button onPress={onHelp}>Help &amp; accessibility</Button>
      </Column>
    </Screen>
  );
}
```

## To this

```rust
pub fn build<B: ui::UiBuilder>(b: &mut B) -> Result<Nodes<B::Node>, B::Error> {
    let n1 = b.text("Welcome")?; // welcome.tsx:17:9 <Text>
    let n2 = b.text("Touch Start to begin.")?; // welcome.tsx:18:9 <Text>
    let n4 = b.spacer(None)?; // welcome.tsx:20:9 <Spacer>
    let n5 = b.button("Start", actions::START)?; // welcome.tsx:21:9 <Button>
    let n6 = b.button("Help & accessibility", actions::HELP)?; // welcome.tsx:24:9 <Button>
    let n0 = b.column(ui::Layout::new().gap(super::tokens::SPACE_SM).padding(super::tokens::SPACE_LG), &[n1, n2, n4, n5, n6])?; // welcome.tsx:16:7 <Column>
    Ok(Nodes { root: n0, title: n1, start: n5 })
}
```

Each node points back to the TSX it came from. Each converted screen is then
drawn by NoWayOS's renderer and must match its approved image, pixel for
pixel. (Shortened from `fixtures/welcome.tsx` and its generated Rust.)

## Why the Screen Compiler

- **More secure.** There is no JavaScript, browser engine or WebView on the
  device, so the most attacked and most patched software in a typical kiosk
  is simply not there. Generated code has no `unsafe`, no `unwrap` and no
  `panic`.
- **More efficient.** Screens become static Rust: nothing to download,
  interpret or lay out by a web engine on the device, just NoWayOS's small UI
  core.
- **Easier to work with.** Keep React's components, props, design tokens and
  your editor. Every error names its `file:line:column`.
- **Exact or nothing.** The Screen Compiler converts its subset exactly and refuses
  everything else, with the line and column; it never approximates. The same
  screen gives byte-identical Rust on any machine.

## What it converts

Screens built from `Screen`, `Column`, `Row`, `Text`, `Button`, `TextInput`,
`Checkbox`, `RadioGroup`, `Spacer`, `Card`, `Notice`, `HeaderBar`,
`ProgressBar` and `Icon`, imported from the Screen Compiler's kit; design tokens as
constants; callback props that become actions; text the application sets at
run time; and parts the application shows or hides. The full subset, element
by element: [DEVELOPING.md](DEVELOPING.md).

## Status

**In development.** Subset v0 and subset v1 - every control a form-heavy
kiosk needs - are built. The subset now grows towards parity with the React
and CSS that real interfaces use, as far as converting exactly allows:
whatever needs JavaScript or a browser on the device stays out. The
generated Rust targets a provisional contract with NoWayOS's UI builder,
checked on every change by building each converted screen against the real
UI runtime and drawing it. Expect the subset and the contract to grow and
change.

## Try it

The Screen Compiler's dev container and its contract check come from NoWayOS, so clone
the two side by side:

```sh
mkdir nowayos-work && cd nowayos-work
git clone https://github.com/NoWayOS/nowayos.git
git clone https://github.com/NoWayOS/screen-compiler.git screen-compiler
make -C nowayos container                    # build the pinned dev image once (about 4 minutes)
cd screen-compiler
make test                                    # unit, property, snapshot and CLI tests
make run ARGS="check fixtures/welcome.tsx"   # check one screen against the subset
make run ARGS="audit fixtures"               # how much of a folder of TSX converts
make contract-check                          # every screen built against NoWayOS's UI and drawn
```

`make` lists every target. Building, the subset in full and the contract:
[DEVELOPING.md](DEVELOPING.md).

**Run the Screen Compiler on your own screens only.** It is a build tool, not a sandbox:
convert TSX you trust, as you would compile code you trust.

## Licence

**MIT**: see [`LICENSE`](LICENSE). **The Rust that the Screen Compiler generates from your
screens is yours**, to license as you wish.

## Contributing

Start from an issue, sign off every commit, and pass the gates Defence Agent
writes down: [CONTRIBUTING.md](CONTRIBUTING.md). Instructions for AI
assistants: [AGENTS.md](AGENTS.md).

## Security

Please report vulnerabilities privately: [SECURITY.md](SECURITY.md).

## Part of NoWayOS

NoWayOS builds devices from only what their one job needs.
**[NoWayOS](https://github.com/NoWayOS/nowayos)** is its open
Single Purpose OS, the **Screen Compiler** brings React screens
onto it, and the **[Showcase](https://github.com/NoWayOS/showcase)** shows what
you can build. NoWayOS is sponsored by Adappt.ai.

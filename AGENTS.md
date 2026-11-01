# Instructions for AI agents

For any AI coding assistant working in this repository. Read
[CONTRIBUTING.md](CONTRIBUTING.md) too: these rules come from it and from the
gates in [Defence Agent](https://github.com/NoWayOS/defence-agent).

The Screen Compiler is a build-time tool: it converts a declared subset of
React/TSX into Rust for NoWayOS's UI runtime. It runs on developer machines
and in CI, never on a device. **Run it only on screens you trust**: it is a
compiler, not a sandbox.

## Building

- **Install nothing on the host:** no toolchains, no `rustup`, no
  `cargo install`, no package managers. Every `make` target runs in NoWayOS's
  dev container, built in the NoWayOS clone beside this one.
- Never run plain `cargo` on the host; the host loop is opt-in, for a person
  who already has Rust 1.93.1: `make test IN=host CARGO="cargo +stable"`.
- Before proposing a change: `make test`, `make lint`, then `make ci`.

## Rules the code keeps

- **Never approximate.** Anything outside the subset is reported with
  `file:line:column`; with any error, no IR and no Rust is produced. Checking
  and lowering stay one walk (`screen-compiler-analyse`).
- Only `screen-compiler-tsx` touches the parser (oxc); everything else uses
  the Screen Compiler's own syntax tree.
- The generated Rust is deterministic (no paths, times or tool versions), pure
  ASCII, `no_std`-friendly, and free of `unwrap`, `panic!` and `unsafe`. It
  targets the contract in `crates/screen-compiler-codegen-rust/src/contract.rs`:
  change the contract and the generator together.
- Snapshots change only on purpose (`make bless`, then review the diff).
  Never approve an image in `contract-check/golden/`: a person does.
- Screen Compiler's own crates never depend on NoWayOS; only `contract-check/`
  does.
- `unsafe` is forbidden; no `unwrap`, `expect`, `panic!` or unchecked indexing
  or arithmetic outside tests; every `#[allow]` gives a reason.
- Anything printed that comes from input goes through `escape_special`.

## Licences

- Our code is MIT; every source file starts with an `SPDX-License-Identifier`
  line, and no file carries a copyright line. Never put licence text in
  generated code: it belongs to whoever runs the tool.
- Add a dependency only if its whole tree is MIT, Apache-2.0, BSD-2-Clause,
  BSD-3-Clause or ISC (`make deny` checks), and state its licence. For anything
  else, stop and ask a maintainer.

## Changes and pull requests

- Never weaken a gate to make a change pass: no deleted or skipped tests, no
  new allowlist entries or `deny.toml` exceptions without saying why, no edits
  to the files Defence Agent's `OWNERS.md` lists as gate-owned.
- Do not add a `Signed-off-by` line for a person: the person who submits the
  change signs it off (`git commit -s`).
- Say in the pull request that an assistant helped, and with what.
- Fixtures are synthetic screens written for the Screen Compiler. No secrets.

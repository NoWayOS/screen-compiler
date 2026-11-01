# Contributing to the Screen Compiler

Thank you for helping. The Screen Compiler turns a declared subset of React
and TSX into Rust for NoWayOS's UI core, at build time. Every change to it is
judged by the same gates as NoWayOS's, written down in
[Defence Agent](https://github.com/NoWayOS/defence-agent); read its `GATES.md`
before your first pull request. This page is the short version.

## Before you start

- **Start from an issue** a maintainer has accepted (gate T1). Open one with
  the bug or feature template, or say on an existing one that you would like
  to take it.
- **A security problem is never an issue:** report it privately, as
  [SECURITY.md](SECURITY.md) says.
- Read [DEVELOPING.md](DEVELOPING.md): the subset, the contract with NoWayOS's
  UI builder, and the commands.

## Build and check

Everything runs in NoWayOS's dev container, so clone NoWayOS beside this
repository, as the README's "Try it" shows. Then:

```sh
make -C ../nowayos container   # the dev image, once
make test lint                 # while you work
make ci                        # before you open a pull request
```

`make ci` is the gates' build stage: a pull request must pass it.

## What a change keeps

- **Never approximate.** Anything outside the subset is refused with its
  `file:line:column`; with any error, no Rust is produced.
- The generated Rust is deterministic, pure ASCII, `no_std`-friendly and free
  of `unwrap`, `panic!` and `unsafe`. A change to the contract changes the
  generator with it.
- Snapshots change only on purpose: `make bless`, then review the diff. An
  approved image (`contract-check/golden/`) changes only when a person has
  looked at it and approved it.
- Parsers reject oversized input early, bound their recursion, return typed
  errors and never panic; they have property tests.
- Dependencies are MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause or ISC. Anything
  else needs a maintainer's written agreement first: ask in the issue.
- No secrets, keys or credentials anywhere. Fixtures are synthetic screens,
  written for the Screen Compiler.

## Pull requests

- Keep each one small and about one thing, with its tests.
- **Every commit carries a `Signed-off-by` line** (`git commit -s`): with it
  you certify the [Developer Certificate of Origin 1.1](https://developercertificate.org/),
  that you wrote the change or have the right to submit it under the
  project's licence. Gate P1 fails a commit without one.
- Fill in the pull-request template, AI assistance included.
- Your contribution is licensed as the project is, MIT
  ([README](README.md#licence)).

## AI assistants

You may use one. Say so in the pull request, and which parts it wrote: you
answer for every line as if you had written it. [AGENTS.md](AGENTS.md) holds
the instructions for an assistant working in this repository.

## For maintainers

Until outside contributors join, the maintainers' own commits need no
`Signed-off-by` line; every contributor's commit does. An assistant never
signs off on anyone's behalf.

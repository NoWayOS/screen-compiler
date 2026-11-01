# SPDX-License-Identifier: MIT
"""Fails when a third-party crate in contract-check/Cargo.lock has a version
that ../nowayos/Cargo.lock does not pin: the approved images must be drawn by
the renderer NoWayOS itself builds, embedded-graphics and all.

Run from the screen-compiler directory: python3 contract-check/same-versions.py
"""

import re
import sys

PACKAGE = re.compile(r'name = "([^"]+)"\nversion = "([^"]+)"\nsource = "registry')


def registry(path):
    """The (name, version) of every crates.io package a Cargo.lock pins."""
    with open(path, encoding="utf-8") as lock:
        return set(PACKAGE.findall(lock.read()))


ours = registry("contract-check/Cargo.lock")
nowayos = registry("../nowayos/Cargo.lock")
differ = sorted(ours - nowayos)
if differ:
    def theirs(name):
        return ", ".join(sorted(v for n, v in nowayos if n == name)) or "none"

    sys.exit(
        "error: contract-check/Cargo.lock differs from ../nowayos/Cargo.lock: "
        + "; ".join("%s %s (nowayos: %s)" % (n, v, theirs(n)) for n, v in differ)
        + " - pin NoWayOS's versions with cargo update --manifest-path"
        " contract-check/Cargo.toml -p NAME --precise VERSION, then commit the lock"
    )
print(
    "same-versions: PASS - contract-check/Cargo.lock pins NoWayOS's versions of"
    " its %d third-party crates" % len(ours)
)

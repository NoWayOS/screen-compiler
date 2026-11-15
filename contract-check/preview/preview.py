#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""make preview: one TSX file as pictures, drawn by NoWayOS's renderer.

Converts the file with Screen Compiler, builds contract-check/preview around
the Rust that gives, and runs it: a PNG for each screen in the file, in the
output directory. A file that does not convert prints Screen Compiler's
diagnostics and gives no picture.

With --watch it then waits: whenever the file, or any .tsx, .ts or .css file
in its folder or below (the parts a screen may import), is saved, it draws
again and says how long that took. Saving is noticed by reading modification
times, which works across a container's bind mount where file events do not.
A save that does not convert leaves the last pictures as they were.

Run from the repository's root, by the Makefile. Python 3, standard library
only.
"""
import argparse
import os
import shlex
import shutil
import subprocess
import sys
import time

# How often --watch looks for a save, in seconds.
EVERY = 0.2
# The most files --watch will look at.
MOST = 2000
WATCHED = (".tsx", ".ts", ".css")


def say(text):
    print(text, flush=True)


def target(workspace):
    """Where cargo puts what it builds for `workspace` ("" for the root's)."""
    return os.environ.get("CARGO_TARGET_DIR") or os.path.join(workspace, "target")


def cargo_build(cargo, locked, extra, env=None):
    """Builds quietly; cargo's own errors are the output. True if it built."""
    command = [*cargo, "build", "--quiet", *locked, *extra]
    return subprocess.run(command, env=env, check=False).returncode == 0


class Preview:
    def __init__(self, args):
        self.args = args
        self.cargo = shlex.split(args.cargo)
        self.locked = ["--locked"] if args.locked else []
        stem = os.path.splitext(os.path.basename(args.file))[0]
        self.converted = os.path.join(target("contract-check"), "preview-converted")
        self.rust = os.path.join(self.converted, stem + ".rs")
        self.last = None

    def tool(self):
        """Builds Screen Compiler itself, once."""
        if not cargo_build(self.cargo, self.locked, ["-p", "screen-compiler-cli"]):
            sys.exit(1)
        return os.path.join(target(""), "debug", "screen-compiler")

    def draw(self, tool):
        """Converts, builds and draws. True if the pictures were written."""
        command = [tool, "convert"]
        if self.args.pictures:
            command += ["--pictures", self.args.pictures]
        converted = subprocess.run([*command, self.args.file], stdout=subprocess.PIPE, check=False)
        if converted.returncode != 0:
            return False
        if converted.stdout != self.last:
            shutil.rmtree(self.converted, ignore_errors=True)
            os.makedirs(self.converted)
            with open(self.rust, "wb") as handle:
                handle.write(converted.stdout)
            env = dict(os.environ, SCREEN_COMPILER_PREVIEW=os.path.abspath(self.converted))
            built = cargo_build(
                self.cargo,
                self.locked,
                ["--manifest-path", "contract-check/Cargo.toml", "-p", "screen-compiler-preview"],
                env,
            )
            if not built:
                self.last = None
                return False
            self.last = converted.stdout
        program = os.path.join(target("contract-check"), "debug", "screen-compiler-preview")
        return subprocess.run([program, self.args.out, *self.args.view], check=False).returncode == 0

    def stamps(self):
        """The modification time of every file a save could change."""
        found = {}
        for extra in (self.args.file, self.args.pictures):
            if extra:
                found[extra] = mtime(extra)
        for folder, folders, files in os.walk(os.path.dirname(self.args.file) or "."):
            folders[:] = [name for name in folders if not name.startswith(".") and name != "node_modules"]
            for name in files:
                if name.endswith(WATCHED) and len(found) < MOST:
                    path = os.path.join(folder, name)
                    found[path] = mtime(path)
        return found


def mtime(path):
    try:
        return os.stat(path).st_mtime_ns
    except OSError:
        return None


def main():
    parser = argparse.ArgumentParser(description="One TSX file as pictures.")
    parser.add_argument("--file", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--pictures", default="")
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--locked", action="store_true")
    parser.add_argument("--watch", action="store_true")
    parser.add_argument("view", nargs="*", help="after --: what to show (see `make help`)")
    args = parser.parse_args()
    if not os.path.isfile(args.file):
        sys.exit(f"error: {args.file}: no such file (a path from this repository's root, or ../REPO/...)")

    preview = Preview(args)
    tool = preview.tool()
    drawn = preview.draw(tool)
    if not args.watch:
        sys.exit(0 if drawn else 1)

    say(f"preview: watching {args.file} and the files beside it; Ctrl-C stops")
    seen = preview.stamps()
    try:
        while True:
            time.sleep(EVERY)
            now = preview.stamps()
            if now == seen:
                continue
            seen = now
            started = time.monotonic()
            if preview.draw(tool):
                say(f"preview: drawn again in {time.monotonic() - started:.1f} s")
            else:
                say("preview: not drawn; the pictures are the last that were")
    except KeyboardInterrupt:
        say("")


if __name__ == "__main__":
    main()

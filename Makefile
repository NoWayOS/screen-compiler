# SPDX-License-Identifier: MIT
# Screen Compiler: developer commands. `make` (or `make help`) lists them.
#
# Every target runs in the NoWayOS dev container by default, through the
# shared helper in the sibling nowayos clone (nowayos/tools/make/container.mk),
# so no host Rust is needed. Build the image once with
# `make container` in nowayos/. `make ci` runs secrets, lint, test, doc,
# contract-check, no-nowayos, deny and sbom there.
#
# The everyday targets - fmt, lint, test, doc, fixtures, bless, run,
# no-nowayos and clean - can run on the host instead with IN=host, using the
# toolchain pinned in rust-toolchain.toml (Rust 1.93.1), or the host's stable
# toolchain where that already is 1.93.1: `make test IN=host CARGO="cargo
# +stable"`. Nothing here installs a toolchain: RUSTUP_AUTO_INSTALL=0 below
# makes rustup report a missing toolchain instead of downloading it. deny,
# contract-check and bless-images always run in the container, which has
# cargo-deny and the aarch64-unknown-none target, so the approved images are
# always drawn there; reproducible runs from the host and uses both.
#
# Written for GNU Make 3.81 (the version macOS ships) and later.

CARGO ?= cargo
# Never let a cargo, rustc or rustup call made from here install anything.
RUSTUP_AUTO_INSTALL ?= 0
export RUSTUP_AUTO_INSTALL
# `make ci` sets --locked so the committed Cargo.lock is exactly what is checked.
LOCKED ?=
SCREEN_COMPILER = $(CARGO) run $(LOCKED) --quiet -p screen-compiler-cli --
FIXTURES := $(sort $(wildcard fixtures/*.tsx))
UNSUPPORTED := $(sort $(wildcard fixtures/unsupported/*.tsx))
# contract-check/: every generated screen against the real nowayos-ui,
# drawn, and matched with its approved image in contract-check/golden/.
CONTRACT := contract-check/Cargo.toml
CONVERTED := $(CURDIR)/target/contract-check-converted
# make reproducible: the fixtures converted on the host and in the container.
REPRODUCIBLE := target/reproducible
# make sbom writes a CycloneDX SBOM for each workspace here.
SBOM_DIR := build/sbom

# Prints which NoWayOS commit the screens are built and drawn against.
define nowayos_revision
	@if ! git -C ../nowayos rev-parse --git-dir >/dev/null 2>&1; then \
		echo "error: make $@ needs a nowayos clone beside screen-compiler (../nowayos)" >&2; exit 1; \
	fi; \
	rev=$$(git -C ../nowayos rev-parse --verify --quiet --short HEAD) || rev="no commits yet"; \
	if [ -n "$$(git --no-optional-locks -C ../nowayos status --porcelain)" ]; then rev="$$rev, with local changes"; fi; \
	echo "Building generated screens against nowayos: $$rev"
endef

# Converts every fixture into the directory $(1), one .rs file each.
define convert_fixtures
	@rm -rf $(1) && mkdir -p $(1)
	@for f in $(FIXTURES); do \
		echo "==> screen-compiler convert $$f"; \
		$(SCREEN_COMPILER) convert $$f > $(1)/$$(basename $$f .tsx).rs || exit 1; \
	done
endef

# Shared dev-container helper: IN, ENGINE, IMAGE, WS, IN_CONTAINER. Optional, so
# a missing nowayos clone never breaks the host targets (with IN=host).
# `make run ARGS="check fixtures/welcome.tsx"` runs the screen-compiler tool itself.
-include ../nowayos/tools/make/container.mk

.DEFAULT_GOAL := help
.PHONY: help fmt lint test doc fixtures bless run contract-check bless-images reproducible \
	convert-into no-nowayos deny sbom secrets ci ci-local clean

help:
	@echo "Screen Compiler make targets (in the dev container; IN=host runs the everyday ones on the host)"
	@echo "  make fmt       format the code (cargo fmt)"
	@echo "  make lint      cargo fmt --check, then clippy on all targets with warnings denied"
	@echo "  make test      unit, property, snapshot and command-line tests"
	@echo "  make doc       build the API docs, warnings as errors"
	@echo "  make fixtures  run screen-compiler check and convert on every fixture"
	@echo "  make bless     rewrite the snapshot files after a deliberate change; review the diff"
	@echo "  make run ARGS=\"check FILE.tsx\"   run the screen-compiler tool (ARGS: its command line)"
	@echo "  make contract-check   every generated screen built against the real nowayos-ui (../nowayos), host and aarch64-unknown-none; each drawn, and each fixture's frame matched with its approved image (contract-check/golden/)"
	@echo "  make bless-images     redraw the approved images after a deliberate change, in the dev container; review them by eye"
	@echo "  make reproducible     from the host: each fixture converts to byte-identical Rust on the host and in the dev container, and draws on the host exactly as approved"
	@echo "  make no-nowayos       Screen Compiler's own crates depend on no NoWayOS crate (cargo metadata)"
	@echo "  make deny      licence, source and advisory checks with cargo-deny (dev container only)"
	@echo "  make sbom      a CycloneDX SBOM for each workspace: $(SBOM_DIR)/screen-compiler.cdx.json, screen-compiler-contract-check.cdx.json (dev container only)"
	@echo "  make secrets   scan the tree and every commit for keys and credentials, offline (NoWayOS's tools/secrets-scan; reviewed false positives in secrets-allow.toml; dev container only)"
	@echo "  make ci        secrets, lint, test, doc, contract-check, no-nowayos, deny and sbom inside the dev container, with --locked"
	@echo "  make clean     remove build output (the container's, or the host's with IN=host)"
	@echo "Current settings: IN=\"$(IN)\" CARGO=\"$(CARGO)\" ENGINE=\"$(ENGINE)\" IMAGE=\"$(IMAGE)\""

# The everyday targets run here inside the dev container or with IN=host;
# otherwise they start the container.
ifneq ($(NOWAYOS_CONTAINER)$(filter host,$(IN)),)
fmt:
	$(CARGO) fmt --all

lint:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace --all-targets $(LOCKED) -- -D warnings

test:
	$(CARGO) test --workspace $(LOCKED)

doc:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps $(LOCKED)

# Supported fixtures must check and convert; unsupported ones must be refused
# (exit status 1) with their diagnostics.
fixtures:
	@for f in $(FIXTURES); do \
		echo "==> screen-compiler check $$f"; $(SCREEN_COMPILER) check $$f || exit 1; \
		echo "==> screen-compiler convert $$f"; $(SCREEN_COMPILER) convert $$f || exit 1; \
	done
	@for f in $(UNSUPPORTED); do \
		echo "==> screen-compiler check $$f (expected: not supported)"; \
		$(SCREEN_COMPILER) check $$f; status=$$?; \
		if [ $$status -ne 1 ]; then echo "error: expected exit status 1, got $$status" >&2; exit 1; fi; \
	done

# Converts every fixture into DIR: make reproducible's helper, run on the host
# and in the dev container.
convert-into:
	@if [ -z "$(DIR)" ]; then echo "error: make convert-into needs DIR" >&2; exit 1; fi
	$(call convert_fixtures,$(DIR))

# Only the tests that write snapshots, so no other test reads a half-written set.
bless:
	SCREEN_COMPILER_BLESS=1 $(CARGO) test $(LOCKED) -p screen-compiler-codegen-rust --test snapshot -- generated_rust_matches_the_snapshots
	SCREEN_COMPILER_BLESS=1 $(CARGO) test $(LOCKED) -p screen-compiler-analyse --test unsupported_fixtures
	SCREEN_COMPILER_BLESS=1 $(CARGO) test $(LOCKED) -p screen-compiler-analyse --test audit -- the_fixtures_report_is_as_approved

# The screen-compiler tool itself; paths in ARGS are relative to this folder. Silent,
# so its standard output can be redirected to a file.
run:
	@$(SCREEN_COMPILER) $(ARGS)

# Screen Compiler's own crates never depend on NoWayOS: only contract-check/
# does. Fails naming any NoWayOS crate in the main workspace's graph.
no-nowayos:
	@$(CARGO) metadata $(LOCKED) --all-features --format-version 1 | python3 -c 'import json, sys; names = sorted({p["name"] for p in json.load(sys.stdin)["packages"] if p["name"].startswith("nowayos")}); sys.exit("error: the Screen Compiler workspace depends on " + ", ".join(names) + "") if names else print("no-nowayos: PASS - no NoWayOS crate in the Screen Compiler workspace")'

clean:
	$(CARGO) clean
	$(CARGO) clean --manifest-path $(CONTRACT)
else ifdef IN_CONTAINER
fmt lint test doc fixtures bless no-nowayos clean:
	$(IN_CONTAINER) make $@ CARGO=cargo LOCKED="$(LOCKED)"
convert-into:
	$(IN_CONTAINER) make $@ CARGO=cargo LOCKED="$(LOCKED)" DIR='$(DIR)'
run:
	@$(IN_CONTAINER) make --no-print-directory run CARGO=cargo LOCKED="$(LOCKED)" ARGS='$(ARGS)'
else
fmt lint test doc fixtures bless run no-nowayos convert-into clean:
	@echo "error: make $@ needs ../nowayos/tools/make/container.mk - clone nowayos beside screen-compiler - or IN=host to run on the host" >&2
	@exit 1
endif

ci-local: LOCKED := --locked
ci-local: secrets lint test doc contract-check no-nowayos deny sbom

ifeq ($(NOWAYOS_CONTAINER),1)
# Every screen Screen Compiler generates, built against the real nowayos-ui from the
# sibling clone: the checked-in snapshots, and every fixture
# converted now. contract-check/ is a workspace of its own, so Screen Compiler's
# crates never depend on NoWayOS. This prints which NoWayOS commit it
# built against, then lints it; its tests build each screen on a real
# nowayos-ui tree, lay it out at 800x480 and draw it with NoWayOS's renderer,
# and each fixture's frame must match its approved image in
# contract-check/golden/ pixel for pixel; then it builds for
# aarch64-unknown-none. Container only: the host toolchain has no
# aarch64-unknown-none target.
contract-check:
	$(call nowayos_revision)
	@$(CARGO) metadata --manifest-path $(CONTRACT) $(LOCKED) --format-version 1 >/dev/null || { \
		echo "error: contract-check/Cargo.lock no longer matches the NoWayOS crates in ../nowayos: in screen-compiler, run make contract-check without LOCKED to refresh it (after a NoWayOS version change, update the =0.1.0 requirements in contract-check/Cargo.toml first), then commit it" >&2; exit 1; }
	@python3 contract-check/same-versions.py
	$(call convert_fixtures,$(CONVERTED))
	$(CARGO) fmt --manifest-path $(CONTRACT) -- --check
	SCREEN_COMPILER_CONVERTED=$(CONVERTED) $(CARGO) clippy --manifest-path $(CONTRACT) --all-targets $(LOCKED) -- -D warnings
	NOWAYOS_UPDATE_SNAPSHOTS= SCREEN_COMPILER_CONVERTED=$(CONVERTED) $(CARGO) test --manifest-path $(CONTRACT) $(LOCKED)
	SCREEN_COMPILER_CONVERTED=$(CONVERTED) $(CARGO) build --manifest-path $(CONTRACT) $(LOCKED) --target aarch64-unknown-none

# Redraws every fixture's approved image from a fresh conversion, after a
# deliberate change to a fixture, Screen Compiler or NoWayOS's drawing.
# Always in the container, so the approved images are drawn in one place.
bless-images:
	$(call nowayos_revision)
	$(call convert_fixtures,$(CONVERTED))
	NOWAYOS_UPDATE_SNAPSHOTS=1 SCREEN_COMPILER_CONVERTED=$(CONVERTED) $(CARGO) test --manifest-path $(CONTRACT) $(LOCKED)
	@echo "Now a person reviews the changed images in contract-check/golden/ by eye (git status lists them; GitHub and any image viewer show PNG) before they are committed, naming the NoWayOS commit above in the commit message."

# Already inside the dev container: run the checks directly. The NoWayOS
# baseline (lints, profiles, deny settings) first, then cargo-deny.
deny:
	python3 ../nowayos/tools/check-baseline . contract-check
	$(CARGO) deny $(LOCKED) check
	$(CARGO) deny $(LOCKED) --manifest-path $(CONTRACT) check

# A CycloneDX SBOM for each workspace, by NoWayOS's generator
# (../nowayos/tools/sbom), with the rule every repo uses. Screen Compiler is a
# build-time tool that never ships on a device: its SBOM lists what the tool
# is built from as `required`. contract-check/ is CI's test harness, so
# everything in it is marked `excluded`. Dev-dependencies, build-time tools and
# other platforms' dependencies are marked `excluded` too, with the reason.
sbom:
	@mkdir -p $(SBOM_DIR) && { test -f build/.gitignore || printf '*\n' > build/.gitignore; }
	python3 ../nowayos/tools/sbom --workspace . --name screen-compiler --kind application \
		--out $(SBOM_DIR)/screen-compiler.cdx.json
	python3 ../nowayos/tools/sbom --workspace contract-check --name screen-compiler-contract-check \
		--host-only-workspace --out $(SBOM_DIR)/screen-compiler-contract-check.cdx.json

# The secrets scan, by NoWayOS's scanner (../nowayos/tools/secrets-scan),
# with the rule every repo uses: the tree, and every version of
# every file and every commit message on every ref, offline. A finding names
# its file and line, never the text; reviewed false positives go in
# secrets-allow.toml, one exact entry each with its reason. It needs the
# whole history: a shallow clone fails.
secrets:
	python3 ../nowayos/tools/secrets-scan --repo . --allow secrets-allow.toml
ci: ci-local
else ifdef IN_CONTAINER
# On the host: run them in the dev container (the secrets scan with no network).
secrets: override CONTAINER_RUN_ARGS += --network=none
contract-check bless-images deny sbom secrets:
	$(IN_CONTAINER) make $@ CARGO=cargo LOCKED=$(LOCKED)
ci:
	$(IN_CONTAINER) make ci-local CARGO=cargo
else
contract-check bless-images deny sbom secrets ci:
	@echo "error: make $@ needs ../nowayos/tools/make/container.mk - clone nowayos beside screen-compiler" >&2
	@exit 1
endif

# The same fixtures, converted on the host and in the dev container, must
# give byte-identical Rust, and drawn on the host must match the approved
# images drawn in the container. Runs from the host, with the host
# toolchain where that is 1.93.1 (CARGO="cargo +stable"), and the container.
ifeq ($(NOWAYOS_CONTAINER),1)
reproducible:
	@echo "error: make reproducible runs from the host: it compares the host's conversions with the dev container's" >&2
	@exit 1
else ifdef IN_CONTAINER
reproducible:
	$(IN_CONTAINER) make --no-print-directory convert-into DIR=$(REPRODUCIBLE)/container CARGO=cargo LOCKED=--locked
	$(MAKE) --no-print-directory convert-into DIR=$(REPRODUCIBLE)/host IN=host LOCKED=--locked
	@for f in $(FIXTURES); do \
		n=$$(basename $$f .tsx).rs; \
		cmp $(REPRODUCIBLE)/host/$$n $(REPRODUCIBLE)/container/$$n || { \
			echo "error: $$f converts to different Rust on the host and in the dev container" >&2; exit 1; }; \
	done; \
	echo "reproducible: every fixture converts to byte-identical Rust on the host and in the dev container"
	NOWAYOS_UPDATE_SNAPSHOTS= SCREEN_COMPILER_CONVERTED=$(CURDIR)/$(REPRODUCIBLE)/host $(CARGO) test --manifest-path $(CONTRACT) --locked
	@echo "reproducible: PASS - the host's conversions are byte-identical to the container's and draw on the host exactly the approved images that make ci draws in the container"
else
reproducible:
	@echo "error: make $@ needs ../nowayos/tools/make/container.mk - clone nowayos beside screen-compiler" >&2
	@exit 1
endif

.PHONY: check analyze analyze-floor platforms lint-shell format fixtures test-guards \
       build build-native build-wasm \
       compile-macos compile-ios compile-android compile-linux compile-windows compile-wasm compile-natives \
       test test-pkg-native test-unit test-rust \
       test-ops test-ops-native test-ops-web test-ops-opfs test-ops-jspi test-ops-atomics \
       test-example test-example-matrix test-example-macos test-example-linux test-example-windows \
       test-example-android test-example-ios test-example-device \
       test-example-web test-example-web-jspi test-example-web-atomics test-example-web-opfs \
       verify verify-android verify-ios verify-macos \
       verify-linux verify-windows verify-web \
       clean hooks

# ═══════════════════════════════════════════════════════════════════
# SDK resolution
#
# Uses fvm by default (.fvmrc pins to stable). Contributors without
# fvm can override: make check DART=dart FLUTTER=flutter
# ═══════════════════════════════════════════════════════════════════

DART    ?= fvm dart
FLUTTER ?= fvm flutter
CARGO   ?= cargo
TEST_RESULTS_DIR ?= test-results
TIMEOUT := $(if $(CI),--timeout=30x,)
VERBOSE := $(if $(CI),--verbose,)

# ═══════════════════════════════════════════════════════════════════
# § 1 — Gate
# ═══════════════════════════════════════════════════════════════════
#
# make check    Full local gate before PR.

check: lint-shell analyze platforms test-guards test test-example

# make hooks    Activate the repo's git hooks (commit-msg, pre-commit).
#               Run once after cloning — they stay dormant otherwise.
#               Idempotent.
hooks:
	@git config core.hooksPath .githooks
	@echo "✓ git hooks active (core.hooksPath → .githooks)"

# ═══════════════════════════════════════════════════════════════════
# § 2 — Analyze
# ═══════════════════════════════════════════════════════════════════
#
# make analyze  Dart + Rust static analysis. Depends on fixtures:
#               analyzing test/ requires the generated fixtures it
#               imports to exist (gitignored — absent on fresh checkout).

analyze: fixtures
	@DART="$(DART)" FLUTTER="$(FLUTTER)" bash tool/analyze.sh

# make analyze-floor  Resolve to the OLDEST in-range dependencies and analyze
#                     the shipped code (lib bin hook). The wide lower bounds
#                     (e.g. package_config >=2.1.0) are only honest if the code
#                     analyzes against them, not just the newest a fresh build
#                     resolves: the dependency half of "works for some, breaks
#                     for some". Static analysis only, so no fixtures and no
#                     native build. Tests are excluded on purpose; a consumer
#                     sees lib, never your tests. Snapshots and restores the lock
#                     so a local run leaves the tree clean.
analyze-floor:
	@cp pubspec.lock pubspec.lock.floorbak; \
	$(DART) pub downgrade --no-example >/dev/null && $(DART) analyze --fatal-infos lib bin hook; rc=$$?; \
	mv pubspec.lock.floorbak pubspec.lock; \
	$(DART) pub get --no-example >/dev/null 2>&1 || true; \
	exit $$rc

# make platforms  Gate pub.dev platform support: pana (the exact analyzer
#                 pub.dev runs, pinned + radar-tracked) must still report all 6
#                 platforms, else a regression like an unconditional dart:io/ffi
#                 import silently drops web. Shared gate tool/platforms_gate.sh
#                 (canonical in whuppi/ci, stamped into tool/); PANA_VERSION
#                 comes from this repo's tool/versions.env.
platforms:
	@DART="$(DART)" EXPECTED_PLATFORMS="android ios linux macos windows web" bash tool/platforms_gate.sh

# make lint-shell  Shell portability gate: shellcheck + a bash 4.0+ scan
#                  that catches macOS bash 3.2 breaks in scripts and in
#                  workflow run: blocks. Mirrors the CI workflow-lint job.
lint-shell:
	@bash tool/lint_shell.sh

# make format  Format all Dart (root + example), each from its own package root
#              so the resolved language version matches CI. make analyze formats
#              too; this is a standalone format-only pass.
format:
	@$(DART) pub get --no-example >/dev/null
	@( cd example && $(FLUTTER) pub get >/dev/null )
	@$(DART) format lib bin test tool hook
	@( cd example && $(DART) format lib integration_test )

# ═══════════════════════════════════════════════════════════════════
# § 2b — Fixtures + test-suite guards
# ═══════════════════════════════════════════════════════════════════
#
# make fixtures      Generate test fixtures (skips when the stamp matches
#                    the catalog — existence is never proof, the stamp is).
# make test-guards   Mechanical guards over the test suite:
#                      - no dart:io in tests (fixtures are imported Dart
#                        source, so VM and browser consume identical
#                        bytes; exempt: the hybrid asset server, native
#                        process-death tests, and the VM-only
#                        source-parity guards that read repo source)
#                      - no byte-grep content assertions (content claims
#                        go through extract/search/render; %PDF- / ZIP
#                        magic / encrypted-leak checks pass, and a
#                        deliberate raw-bytes check carries an inline
#                        `bytegrep-exempt` marker with its reason; the
#                        builder battery is exempt wholesale — emitted
#                        PDF syntax IS its subject)

fixtures:
	$(DART) run tool/generate_fixtures.dart

# Two static guards. Guard 1: tests run identically on VM and browser, so no
# dart:io; a test that genuinely needs it carries an 'io-exempt: <reason>'
# comment. Guard 2: content claims go through extract/search/render. Its
# exclusions are always-fine idioms (sublist(0, 5) is the %PDF magic check,
# isNot(contains ...)), not per-file opt-outs, so they stay positional;
# bytegrep-exempt is the per-line opt-out, and pdf_builder_battery is excluded
# whole rather than marked 15 times.
test-guards:
	@bad=$$(for f in $$(grep -rln "dart:io" test/ --include="*.dart"); do \
	  grep -q "io-exempt:" "$$f" || echo "$$f"; \
	done); \
	if [ -n "$$bad" ]; then \
	  echo "dart:io in a test. Tests run identically on VM and browser;"; \
	  echo "fixtures are imported Dart source, never file I/O. A test that"; \
	  echo "legitimately needs dart:io carries an 'io-exempt:' comment with"; \
	  echo "the reason. Missing it:"; \
	  echo "$$bad"; exit 1; fi
	@bad=$$(grep -rn "String.fromCharCodes" test/ops/ --include="*.dart" \
	  | grep -v "sublist(0, 5)" | grep -v "isNot(contains" | grep -v "bytegrep-exempt" \
	  | grep -v "pdf_builder_battery.dart" || true); \
	if [ -n "$$bad" ]; then \
	  echo "byte-grep content assertion: content claims go through"; \
	  echo "extract/search/render, or carry a bytegrep-exempt marker"; \
	  echo "with the reason:"; \
	  echo "$$bad"; exit 1; fi

# ═══════════════════════════════════════════════════════════════════
# § 3 — Build (dev iteration)
# ═══════════════════════════════════════════════════════════════════
#
# make build          Both native + WASM.
# make build-native   Native binary for current host.
# make build-wasm     WASM + JS glue.

build: build-native build-wasm

# Triggers the build hook to compile the native binary for the
# current host. Runs a non-existent test name so dart test starts
# (invoking the hook) but no actual test executes. A missing test
# match and a build-hook compile failure both exit non-zero and the
# tee pipe masks the code, so gate on the tooling's failure markers:
# a broken native build must not pass silently.
build-native:
	@tmp=$$(mktemp); \
	$(DART) test test/ops/runners/native_runner_test.dart --concurrency=1 --name='DOES_NOT_EXIST' 2>&1 | tee "$$tmp" || true; \
	if grep -qE 'Building assets for package .* failed|returned with exit code|could not compile' "$$tmp"; then \
		rm -f "$$tmp"; echo "build-native: native build hook failed (see output above)"; exit 1; \
	fi; \
	rm -f "$$tmp"

build-wasm:
	bash tool/compile_rust.sh wasm

# ═══════════════════════════════════════════════════════════════════
# § 4 — Compile (release pipeline — one target per CI job)
# ═══════════════════════════════════════════════════════════════════
#
# make compile-macos     macOS arm64 + x64.
# make compile-ios       iOS device + simulators.
# make compile-android   Android arm64, arm, x64, x86.
# make compile-linux     Linux x64 + arm64 (cross-compile).
# make compile-windows   Windows x64 + arm64.
# make compile-wasm      WASM + JS glue.
# make compile-natives   All native targets the host can build.

compile-macos:
	bash tool/compile_rust.sh macos

compile-ios:
	bash tool/compile_rust.sh ios

compile-android:
	bash tool/compile_rust.sh android

compile-linux:
	bash tool/compile_rust.sh linux

compile-windows:
	bash tool/compile_rust.sh windows

compile-wasm: build-wasm

compile-natives:
	bash tool/compile_rust.sh native

# ═══════════════════════════════════════════════════════════════════
# § 5 — Test
# ═══════════════════════════════════════════════════════════════════
#
# make test              Unit + all ops (native + 3 web modes).
# make test-pkg-native   Unit + native ops only (CI fast gate).
# make test-unit         Types + bridge + runtime + harness (pure Dart, VM).
# make test-ops          All 4 ops runners.
# make test-ops-native   Ops: native FFI.
# make test-ops-web      Ops: all 3 web modes.
# make test-ops-opfs     Ops: web OPFS.
# make test-ops-jspi     Ops: web JSPI.
# make test-ops-atomics  Ops: web Atomics.
# make test-rust         cargo tests for both vendored crates.

test: fixtures test-unit test-ops

test-pkg-native: fixtures test-unit test-ops-native

test-unit:
	@echo "=== Unit: types + io + bridge + runtime + harness ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	$(DART) test $(TIMEOUT) test/types/ test/io/ test/bridge/ test/runtime/ test/harness/ -p vm --concurrency=1 --file-reporter json:$(TEST_RESULTS_DIR)/unit.json

test-ops: fixtures test-ops-native test-ops-web

test-ops-native:
	@echo "=== Ops: Native ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	$(DART) test $(TIMEOUT) test/ops/runners/native_runner_test.dart --concurrency=1 --file-reporter json:$(TEST_RESULTS_DIR)/ops-native.json

test-ops-web: fixtures test-ops-opfs test-ops-jspi test-ops-atomics

test-ops-opfs:
	@echo "=== Ops: Web OPFS ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	$(DART) test $(TIMEOUT) test/ops/runners/web_opfs_runner_test.dart -p chrome --concurrency=1 --file-reporter json:$(TEST_RESULTS_DIR)/ops-opfs.json

test-ops-jspi:
	@echo "=== Ops: Web JSPI ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	$(DART) test $(TIMEOUT) test/ops/runners/web_jspi_runner_test.dart -p chrome --concurrency=1 --file-reporter json:$(TEST_RESULTS_DIR)/ops-jspi.json

test-ops-atomics:
	@echo "=== Ops: Web Atomics ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	$(DART) test $(TIMEOUT) test/ops/runners/web_atomics_runner_test.dart -p chrome-coi --concurrency=1 --file-reporter json:$(TEST_RESULTS_DIR)/ops-atomics.json

# Rust unit + integration tests for both vendored crates. Most of the
# engine's tests sit behind features we don't ship (ml / ocr / fips /
# python / wasm), which need ONNX / pdfium / Python / a wasm target and
# can't run natively here. pdf_oxide reuses build.json's native feature
# set — the SAME source the build and analyze read — plus test-support,
# so a feature added to build.json is tested automatically; office_oxide
# runs default.
#
# Debug symbols dominate Rust test-binary size — a stock CI runner can't hold
# the full-debug build of both crates' test suites even after the free-disk
# capability runs (see the rust job in ci.yml). Strip debuginfo + the
# incremental cache for the test build: every test still compiles and runs
# identically; only interactive backtraces lose symbol names (CI prints the
# assertion output regardless).
test-rust: export CARGO_INCREMENTAL := 0
test-rust: export CARGO_PROFILE_DEV_DEBUG := 0
test-rust: export CARGO_PROFILE_TEST_DEBUG := 0
# public-api + cjk-form-fonts are test-only: shipped builds (build.json
# feature lists) exclude the crate's own wasm/C API surfaces and the
# embedded fallback fonts, but the upstream test suites cover both (the
# CJK flatten tests are self-gated and silently SKIP without the feature),
# so tests compile and run WITH them.
test-rust:
	@echo "=== Rust: pdf_oxide ==="
	$(CARGO) test --manifest-path vendor/pdf_oxide/Cargo.toml \
	  --features "$$(bash tool/compile_rust.sh --features native),test-support,public-api,cjk-form-fonts"
	@echo "=== Rust: office_oxide ==="
	$(CARGO) test --manifest-path vendor/office_oxide/Cargo.toml

# ═══════════════════════════════════════════════════════════════════
# § 6 — Integration tests (example app)
# ═══════════════════════════════════════════════════════════════════
#
# make test-example              matrix + macOS + all 3 web modes.
# make test-example-matrix       host VM: every journey × every device.
# make test-example-macos        macOS desktop.
# make test-example-linux        Linux desktop (needs GTK + display).
# make test-example-windows      Windows desktop.
# make test-example-android      Android (needs booted emulator).
# make test-example-ios          iOS (needs booted simulator).
# make test-example-device       Custom device (DEVICE=emulator-5554).
# make test-example-web          Chrome: all 3 web modes.
# make test-example-web-jspi     Chrome: JSPI mode.
# make test-example-web-atomics  Chrome: Atomics mode.
# make test-example-web-opfs     Chrome: OPFS mode.

# GTK check — CI auto-installs, dev gets error with instructions.
define ensure_gtk
	@command -v pkg-config >/dev/null && pkg-config --exists gtk+-3.0 || { \
		if [ -n "$$CI" ]; then sudo apt-get update -qq && sudo apt-get install -y -qq ninja-build libgtk-3-dev; \
		else echo "Error: libgtk-3-dev not found. Run: sudo apt-get install -y ninja-build libgtk-3-dev"; exit 1; fi; }
endef

# Web test helpers.
define setup_example_web
	@echo "=== Example: clean + setup web assets ==="
	rm -rf example/web/pdf_manipulator
	cd example && $(FLUTTER) pub get && $(FLUTTER) pub run pdf_manipulator:setup --force web
endef

define run_example_web
	@echo "=== Example: Web $(1) ==="
	@./tool/run_web_test.sh $(2) $(FLUTTER)
endef

test-example: test-example-matrix test-example-macos test-example-web

# Host-VM device-matrix journeys — drives the app through every UI
# journey against every device profile in the harness (test/harness/
# device_profiles.dart), the smallest tighter than any CI emulator. The
# on-device suite can't resize the viewport (it hides the UI —
# flutter/flutter#149209), so size proof lives here. No device, runs
# anywhere `flutter test` runs: a small-screen layout regression fails
# locally on every run, never first in CI.
test-example-matrix:
	@echo "=== Example: device matrix (host VM, every profile) ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	cd example && $(FLUTTER) test $(VERBOSE) $(TIMEOUT) test/journeys/ --file-reporter json:../$(TEST_RESULTS_DIR)/example-matrix.json

test-example-macos:
	@echo "=== Example: macOS ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	cd example && $(FLUTTER) test $(VERBOSE) $(TIMEOUT) integration_test/pdf_smoke_test.dart -d macos --file-reporter json:../$(TEST_RESULTS_DIR)/int-macos.json

test-example-linux:
	@echo "=== Example: Linux ==="
	$(call ensure_gtk)
	@mkdir -p $(TEST_RESULTS_DIR)
	cd example && $(FLUTTER) test $(VERBOSE) $(TIMEOUT) integration_test/pdf_smoke_test.dart -d linux --file-reporter json:../$(TEST_RESULTS_DIR)/int-linux.json

test-example-windows:
	@echo "=== Example: Windows ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	cd example && $(FLUTTER) test $(VERBOSE) $(TIMEOUT) integration_test/pdf_smoke_test.dart -d windows --file-reporter json:../$(TEST_RESULTS_DIR)/int-windows.json

# Runs on the connected/booted device. CI boots the emulator via setup-android.
test-example-android:
	@echo "=== Example: Android ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	cd example && $(FLUTTER) test $(VERBOSE) $(TIMEOUT) integration_test/pdf_smoke_test.dart --file-reporter json:../$(TEST_RESULTS_DIR)/int-android.json

# Runs on the booted simulator. CI boots the simulator via setup-ios.
test-example-ios:
	@echo "=== Example: iOS ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	cd example && $(FLUTTER) test $(VERBOSE) $(TIMEOUT) integration_test/pdf_smoke_test.dart --file-reporter json:../$(TEST_RESULTS_DIR)/int-ios.json

test-example-device:
	@echo "=== Example: device=$(DEVICE) ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	cd example && $(FLUTTER) test $(VERBOSE) $(TIMEOUT) integration_test/pdf_smoke_test.dart -d $(DEVICE) --file-reporter json:../$(TEST_RESULTS_DIR)/int-device.json

test-example-web:
	$(call setup_example_web)
	$(call run_example_web,JSPI,jspi)
	$(call run_example_web,Atomics,atomics)
	$(call run_example_web,OPFS,opfs)

test-example-web-jspi:
	$(call setup_example_web)
	$(call run_example_web,JSPI,jspi)

test-example-web-atomics:
	$(call setup_example_web)
	$(call run_example_web,Atomics,atomics)

test-example-web-opfs:
	$(call setup_example_web)
	$(call run_example_web,OPFS,opfs)

# ═══════════════════════════════════════════════════════════════════
# § 7 — Verify (prove release builds work, output thrown away)
# ═══════════════════════════════════════════════════════════════════
#
# Debug and release builds exercise different code paths in the build
# hook; these prove the hook works in release mode.
#
# make verify          All 6 targets.
# make verify-android  Android APK.
# make verify-ios      iOS (no codesign).
# make verify-macos    macOS.
# make verify-linux    Linux (needs GTK).
# make verify-windows  Windows.
# make verify-web      Web (setup + build).

verify: verify-android verify-ios verify-macos verify-linux verify-windows verify-web

verify-android:
	@echo "=== Verify: Android ==="
	cd example && $(FLUTTER) build apk --release $(VERBOSE)
	@bash tool/check_alignment.sh example/build/app/outputs/flutter-apk/app-release.apk

verify-ios:
	@echo "=== Verify: iOS ==="
	cd example && $(FLUTTER) build ios --release --no-codesign $(VERBOSE)

verify-macos:
	@echo "=== Verify: macOS ==="
	cd example && $(FLUTTER) build macos --release $(VERBOSE)

verify-linux:
	@echo "=== Verify: Linux ==="
	$(call ensure_gtk)
	cd example && $(FLUTTER) build linux --release $(VERBOSE)

verify-windows:
	@echo "=== Verify: Windows ==="
	cd example && $(FLUTTER) build windows --release $(VERBOSE)

verify-web:
	$(call setup_example_web)
	@echo "=== Verify: Web ==="
	@FLUTTER="$(FLUTTER)" bash tool/verify_web_gate.sh

# ═══════════════════════════════════════════════════════════════════
# § 8 — Clean
# ═══════════════════════════════════════════════════════════════════
#
# make clean   Full clean — Dart build-hook artifacts, test results, and
#              both vendored crates' cargo target/ (the bulk of the disk
#              use). Run deliberately; the next build recompiles fresh.

clean:
	rm -rf .dart_tool/hooks_runner/ .dart_tool/lib/ .dart_tool/native_assets/ build_output/ test-results/
	rm -rf vendor/pdf_oxide/target/ vendor/office_oxide/target/

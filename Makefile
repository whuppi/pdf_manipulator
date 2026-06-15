.PHONY: check analyze check-deps fixtures test-guards \
       build build-native build-wasm \
       compile-macos compile-ios compile-android compile-linux compile-windows compile-wasm compile-natives \
       test test-pkg-native test-unit \
       test-ops test-ops-native test-ops-web test-ops-opfs test-ops-jspi test-ops-atomics \
       test-example test-example-matrix test-example-macos test-example-linux test-example-windows \
       test-example-android test-example-ios test-example-device \
       test-example-web test-example-web-jspi test-example-web-atomics test-example-web-opfs \
       verify verify-android verify-ios verify-macos \
       verify-linux verify-windows verify-web \
       clean

# ═══════════════════════════════════════════════════════════════════
# SDK resolution
#
# Uses fvm by default (.fvmrc pins to stable). Contributors without
# fvm can override: make check DART=dart FLUTTER=flutter
# ═══════════════════════════════════════════════════════════════════

DART    ?= fvm dart
FLUTTER ?= fvm flutter
TEST_RESULTS_DIR ?= test-results
TIMEOUT := $(if $(CI),--timeout=30x,)
VERBOSE := $(if $(CI),--verbose,)

# ═══════════════════════════════════════════════════════════════════
# § 1 — Gate
# ═══════════════════════════════════════════════════════════════════
#
# make check    Full local gate before PR.

check: analyze test-guards test test-example

# ═══════════════════════════════════════════════════════════════════
# § 2 — Analyze
# ═══════════════════════════════════════════════════════════════════
#
# make analyze  Dart + Rust static analysis. Depends on fixtures:
#               analyzing test/ requires the generated fixtures it
#               imports to exist (gitignored — absent on fresh checkout).

analyze: fixtures
	@DART="$(DART)" FLUTTER="$(FLUTTER)" bash tool/analyze.sh

# ═══════════════════════════════════════════════════════════════════
# § 2a — Dependency report (network; advisory, never fails)
# ═══════════════════════════════════════════════════════════════════
#
# make check-deps   Report dependency drift against the constraint
#                   doctrine: regular deps keep low floors (new majors
#                   → verify+widen), dev deps track latest. Runs daily
#                   in CI as warnings on the Upgrade Check workflow.
#                   Not part of `make analyze` — a gate must not go
#                   red because someone published.

check-deps:
	$(DART) tool/check_deps.dart

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

test-guards:
	@bad=$$(grep -rln "dart:io" test/ --include="*.dart" \
	  | grep -v "test/harness/asset_server.dart" \
	  | grep -v "test/ops/platform/native/" \
	  | grep -v "test/bridge/protocol/wire_sync_test.dart" \
	  | grep -v "test/runtime/web/lane_worker_sync_test.dart" \
	  | grep -v "test/runtime/dumb_edges_test.dart" || true); \
	if [ -n "$$bad" ]; then \
	  echo "dart:io in tests — tests must run identically on VM and"; \
	  echo "browser; fixtures are imported Dart source, never file I/O:"; \
	  echo "$$bad"; exit 1; fi
	@bad=$$(grep -rn "String.fromCharCodes" test/ops/ --include="*.dart" \
	  | grep -v "sublist(0, 5)" | grep -v "isNot(contains" | grep -v "bytegrep-exempt" \
	  | grep -v "pdf_builder_battery.dart" || true); \
	if [ -n "$$bad" ]; then \
	  echo "byte-grep content assertion — content claims go through"; \
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
# (invoking the hook) but no actual test executes.
build-native:
	$(DART) test test/ops/runners/native_runner_test.dart --concurrency=1 --name='DOES_NOT_EXIST' || true

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

test: fixtures test-unit test-ops

test-pkg-native: fixtures test-unit test-ops-native

test-unit:
	@echo "=== Unit: types + bridge + runtime + harness ==="
	@mkdir -p $(TEST_RESULTS_DIR)
	$(DART) test $(TIMEOUT) test/types/ test/bridge/ test/runtime/ test/harness/ -p vm --concurrency=1 --file-reporter json:$(TEST_RESULTS_DIR)/unit.json

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
	cd example && $(FLUTTER) test $(VERBOSE) $(TIMEOUT) integration_test/pdf_smoke_test.dart -d $(DEVICE)

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
	cd example && $(FLUTTER) build web --release $(VERBOSE)

# ═══════════════════════════════════════════════════════════════════
# § 8 — Clean
# ═══════════════════════════════════════════════════════════════════
#
# make clean   Remove generated build + test-result artifacts.

clean:
	rm -rf .dart_tool/hooks_runner/ .dart_tool/lib/ .dart_tool/native_assets/ build_output/ test-results/

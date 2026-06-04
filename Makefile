.PHONY: check analyze build build-native build-wasm \
       test test-unit test-ops test-ops-native test-ops-web test-ops-opfs test-ops-jspi test-ops-atomics \
       test-example test-example-macos test-example-linux test-example-windows test-example-android test-example-ios test-example-device \
       test-example-web test-example-web-jspi test-example-web-atomics test-example-web-opfs \
       compile compile-natives compile-wasm \
       clean

# ── SDK resolution ──────────────────────────────────────────────────
#
# Uses fvm by default (.fvmrc pins to stable). Contributors without fvm
# can override: make check DART=dart FLUTTER=flutter
DART    ?= fvm dart
FLUTTER ?= fvm flutter

# ── Targets ─────────────────────────────────────────────────────────
#
# DEV (local iteration)
# ─────────────────────
# make check               full gate: analyze + unit + all 4 ops runners + example
# make analyze             analyze package + example (--fatal-infos)
# make build               build native (Rust FFI) + web (WASM)
# make build-native        compile Rust native library via dart build hook
# make build-wasm          compile Rust WASM + wasm-bindgen + wasm-opt
#
# make test                unit + all 4 ops runners
# make test-unit           types + transport protocol tests (pure Dart, fast)
# make test-ops            all 4 ops runners (native + 3 web modes)
# make test-ops-native     ops: native FFI
# make test-ops-web        ops: all 3 web modes
# make test-ops-opfs       ops: web OPFS
# make test-ops-jspi       ops: web JSPI
# make test-ops-atomics    ops: web Atomics
#
# make test-example             example integration tests (native + all 3 web modes)
# make test-example-native      example: macOS
# make test-example-web         example: Chrome (all 3 web modes)
# make test-example-web-jspi    example: Chrome JSPI mode
# make test-example-web-atomics example: Chrome Atomics mode
# make test-example-web-opfs    example: Chrome OPFS mode
#
# RELEASE (CI calls these — single source of truth for compile logic)
# ────────────────────────────────────────────────────────────────────
# make compile-natives     compile Rust for all targets the host can build
# make compile-wasm        compile Rust → WASM + JS glue (release binary)
#
# make clean               wipe build caches
#
# Timeouts: NO global --timeout flag. Every test declares its own
# timeout via Dart's test(timeout: Timeout(...)) parameter. This is
# the performance contract — each test knows how long it should take.

# ── Gate ────────────────────────────────────────────────────────────

check: analyze test test-example

# ── Analyze ─────────────────────────────────────────────────────────

analyze:
	@DART="$(DART)" FLUTTER="$(FLUTTER)" bash tool/analyze.sh

# ── Build (dev — triggers build hook for the current platform) ──────

build: build-native build-wasm

build-native:
	$(DART) test test/ops/smoke_test.dart --concurrency=1 --name='DOES_NOT_EXIST' || true

build-wasm:
	bash tool/compile_rust.sh wasm

# ── Compile (release — produces binaries for GitHub Releases) ───────

compile-natives:
	bash tool/compile_rust.sh native

compile-wasm:
	bash tool/compile_rust.sh wasm

# ── Test ────────────────────────────────────────────────────────────

test: test-unit test-ops

# ── Unit tests (pure Dart — types + transport protocol) ─────────────

test-unit:
	@echo "=== Unit: types + transport ==="
	$(DART) test test/types/ test/transport/ -p vm --concurrency=1

# ── Ops tests (full suite on every platform) ────────────────────────

test-ops: test-ops-native test-ops-web

test-ops-native:
	@echo "=== Ops: Native ==="
	$(DART) test test/ops/runners/native_runner_test.dart --concurrency=1

test-ops-web: test-ops-opfs test-ops-jspi test-ops-atomics

test-ops-opfs:
	@echo "=== Ops: Web OPFS ==="
	$(DART) test test/ops/runners/web_opfs_runner_test.dart -p chrome --concurrency=1

test-ops-jspi:
	@echo "=== Ops: Web JSPI ==="
	$(DART) test test/ops/runners/web_jspi_runner_test.dart -p chrome --concurrency=1

test-ops-atomics:
	@echo "=== Ops: Web Atomics ==="
	$(DART) test test/ops/runners/web_atomics_runner_test.dart -p chrome-coi --concurrency=1

# ── Example integration tests ───────────────────────────────────────

test-example: test-example-macos test-example-web

# DEVICE can be overridden: make test-example-device DEVICE=emulator-5554
DEVICE ?= macos

test-example-macos:
	@echo "=== Example: integration tests (macOS) ==="
	cd example && $(FLUTTER) test integration_test/pdf_smoke_test.dart -d macos

test-example-linux:
	@echo "=== Example: integration tests (Linux) ==="
	cd example && $(FLUTTER) test integration_test/pdf_smoke_test.dart -d linux

test-example-windows:
	@echo "=== Example: integration tests (Windows) ==="
	cd example && $(FLUTTER) test integration_test/pdf_smoke_test.dart -d windows

test-example-android:
	@echo "=== Example: integration tests (Android) ==="
	cd example && $(FLUTTER) test integration_test/pdf_smoke_test.dart

test-example-ios:
	@echo "=== Example: integration tests (iOS) ==="
	cd example && $(FLUTTER) test integration_test/pdf_smoke_test.dart

test-example-device:
	@echo "=== Example: integration tests (device=$(DEVICE)) ==="
	cd example && $(FLUTTER) test integration_test/pdf_smoke_test.dart -d $(DEVICE)

# All 3 web modes via flutter drive.
# SharedArrayBuffer enabled via CHROME_EXECUTABLE wrapper (see tool/chrome_with_sab.sh).
# Shared helper: run flutter drive with a given mode.
# CHROME_EXECUTABLE wrapper adds --enable-features=SharedArrayBuffer to the
# app Chrome. This works around flutter drive's WebDriverService not forwarding
# --web-browser-flag to the app Chrome (it only goes to chromedriver's session).
CHROME_SAB := $(CURDIR)/tool/chrome_with_sab.sh

define run_example_web
	@./tool/run_web_test.sh $(2) $(CHROME_SAB) $(FLUTTER)
endef

define setup_example_web
	@echo "=== Example: clean + setup web assets ==="
	rm -rf example/web/pdf_manipulator
	cd example && $(FLUTTER) pub get && $(FLUTTER) pub run pdf_manipulator:setup --force
endef

# All 3 web modes: setup once, run all three sequentially.
test-example-web:
	$(call setup_example_web)
	$(call run_example_web,JSPI,jspi)
	$(call run_example_web,Atomics,atomics)
	$(call run_example_web,OPFS,opfs)

# Individual modes: each does its own setup (for standalone use).
test-example-web-jspi:
	$(call setup_example_web)
	$(call run_example_web,JSPI,jspi)

test-example-web-atomics:
	$(call setup_example_web)
	$(call run_example_web,Atomics,atomics)

test-example-web-opfs:
	$(call setup_example_web)
	$(call run_example_web,OPFS,opfs)

# ── Clean ───────────────────────────────────────────────────────────

clean:
	rm -rf .dart_tool/hooks_runner/ .dart_tool/lib/ .dart_tool/native_assets/ build_output/

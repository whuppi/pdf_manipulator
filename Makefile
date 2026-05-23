.PHONY: check test test-types test-protocol test-ops test-ops-web test-transport-native test-atomics test-opfs test-all wasm analyze

# Quick check: analyze + all non-browser tests
check: analyze test

# Analyze Dart code
analyze:
	dart analyze lib/ test/

# All non-browser tests
test: test-types test-protocol test-ops test-transport-native

# Types — pure Dart data class tests
test-types:
	@echo "=== types ==="
	@dart test test/types/ --timeout 10s

# Protocol — pure Dart encoding tests
test-protocol:
	@echo "=== protocol ==="
	@dart test test/protocol/ --timeout 10s

# Shared ops through NativeBridge (phone/desktop path)
test-ops:
	@echo "=== ops (native_runner) ==="
	@dart test test/ops/native_runner_test.dart --timeout 30s

# Native transport — condvar timeout, slow source
test-transport-native:
	@echo "=== transport/native ==="
	@dart test test/transport/native/ --timeout 30s

# Shared ops through WebBridge (browser path) — same tests, different bridge
test-ops-web:
	@echo "=== ops (web_runner) ==="
	@dart test test/ops/web_runner_test.dart -p chrome --timeout 90s

# Web transport: Atomics readAt chain
test-atomics:
	@echo "=== transport/atomics ==="
	@dart test test/transport/web/atomics_test.dart -p chrome-coi --timeout 30s

# Web transport: OPFS coordinator pipeline
test-opfs:
	@echo "=== transport/opfs ==="
	@dart test test/transport/web/opfs_pipeline_test.dart -p chrome --timeout 30s

# Everything — all categories, both paths
test-all: test test-ops-web test-atomics test-opfs

# Rebuild WASM binary from vendor/pdf_oxide
wasm:
	bash tool/build_wasm.sh

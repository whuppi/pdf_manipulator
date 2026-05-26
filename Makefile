.PHONY: analyze test test-web test-all check wasm

# ── Local quality gate (run before commit) ─────────────────────────
#
#   make check       — analyze + native + web tests (the full gate)
#   make test        — native tests only (fast iteration)
#   make test-web    — web tests only (needs Chrome)
#   make analyze     — static analysis only

check: analyze test test-web

# ── Analysis ───────────────────────────────────────────────────────

analyze:
	dart analyze .

# ── Tests ──────────────────────────────────────────────────────────

test:
	dart test --timeout=120s

test-web:
	dart test test/ops/web_runner_test.dart -p chrome --timeout=120s
	dart test test/transport/web/atomics_test.dart -p chrome-coi --timeout=60s
	dart test test/transport/web/opfs_pipeline_test.dart -p chrome --timeout=60s

# ── Build ──────────────────────────────────────────────────────────

wasm:
	bash tool/build_wasm.sh

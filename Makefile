.PHONY: check test test-fast test-web build build-wasm clean

# make check      — full gate before commit
# make test       — native tests
# make test-fast  — native, skip stress
# make test-web   — web tests (Chrome)
# make build      — compile native + WASM
# make build-wasm — WASM only
# make clean      — wipe build caches

check: build test test-web
	@dart analyze .

test:
	@dart test --timeout=120s --reporter json | python3 tool/parse_test_json.py

test-fast:
	@dart test --timeout=60s --exclude-tags=stress --reporter json | python3 tool/parse_test_json.py

test-web:
	@dart test test/ops/web_runner_test.dart -p chrome --timeout=120s --reporter json | python3 tool/parse_test_json.py
	@dart test test/transport/web/atomics_test.dart -p chrome-coi --timeout=60s --reporter json | python3 tool/parse_test_json.py
	@dart test test/transport/web/opfs_pipeline_test.dart -p chrome --timeout=60s --reporter json | python3 tool/parse_test_json.py

build:
	@bash tool/build_wasm.sh

build-wasm:
	@bash tool/build_wasm.sh

clean:
	rm -rf .dart_tool/hooks_runner/ .dart_tool/lib/ .dart_tool/native_assets/

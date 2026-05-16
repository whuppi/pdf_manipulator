.PHONY: check test test-web bindings

check:
	dart analyze .
	dart test
	dart test test/web/web_smoke_test.dart -p chrome --timeout=120s

test:
	dart test

test-web:
	dart test test/web/web_smoke_test.dart -p chrome --timeout=120s

bindings:
	dart run ffigen --config ffigen.yaml

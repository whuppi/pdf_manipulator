# pdf_manipulator example — trimmed engine

The same app as [`../example/`](../example/), run under a **trimmed**
engine. It is a build-config shell, not a second app: the only authored
code is this package's `pubspec.yaml`, a two-line `lib/main.dart` that
re-runs the real example, and one integration test that asserts the
trimmed contract. All widgets and journeys come from `../example/`, so
the two flavors cannot drift apart.

## What makes it trimmed

One pubspec entry — the switch a real consumer flips:

```yaml
hooks:
  user_defines:
    pdf_manipulator:
      trim:
        keep: [render]
```

`user_defines` bind to the build-root package, so launching from *this*
directory makes the build hook compile the engine with only the render
capability (plus core, which is always included). A custom feature set
has no prebuilt binary, so the first build compiles from the vendored
source — it needs the Rust toolchain (https://rustup.rs) and takes a
few minutes once; cargo caches the rest.

## What the smoke test proves

`integration_test/trimmed_smoke_test.dart` runs against the real
trimmed binary and asserts the whole contract:

- core ops work (parse, build, edit, fill, flatten)
- the kept capability works (render streams pages)
- every excluded capability (extract, pdfa, office) answers the typed
  not-enabled error — never a crash, never a silent no-op

In the running app the same contract is visible by hand: capability
cards outside the keep-list surface that error as the op status.

## Run it

```sh
cd example_trimmed
flutter run                            # native: compiles the trimmed engine
flutter test integration_test -d macos # the trimmed-contract smoke

# web needs the trimmed wasm first (run from the app you serve):
dart run pdf_manipulator:setup --trim
```

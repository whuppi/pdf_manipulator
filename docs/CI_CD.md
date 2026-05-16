# CI/CD

Two audiences. Two paths. One build hook.

**Contributors** (cloned repo with `vendor/pdf_oxide/` submodule) → hook compiles from source via `cargo build`. Always fresh. Requires Rust (`rustup.rs`).

**Consumers** (installed from pub.dev, no `vendor/` directory) → hook downloads pre-built binary from GitHub Releases. Zero toolchain required.

The fork is automatic: `vendor/pdf_oxide/Cargo.toml` exists on disk → compile. Doesn't exist → download. Version read from `pubspec.yaml` — nothing hardcoded.

---

## How it works end-to-end

### A Flutter developer adds pdf_manipulator to their app

```yaml
# their pubspec.yaml
dependencies:
  pdf_manipulator: ^1.0.0
```

They run `flutter build apk`. Flutter sees `hooks: build: true` in the package's pubspec. It invokes `hook/build.dart` once per target architecture (arm64, arm, x64, x86 for Android). Each invocation receives `targetOS=android` and `targetArchitecture=arm64` etc.

The hook checks: does `vendor/pdf_oxide/Cargo.toml` exist? No — this developer installed from pub.dev, the git submodule isn't included. So it downloads `android-arm64-libpdf_oxide.so` from the GitHub Release matching the package version. Done. No Rust. No cargo. No NDK fiddling.

### A contributor clones the repo and runs tests

```bash
git clone --recursive https://github.com/whuppi/pdf_manipulator
cd pdf_manipulator
dart test
```

The hook checks: does `vendor/pdf_oxide/Cargo.toml` exist? Yes — the submodule is there. So it runs `cargo build --target aarch64-apple-darwin --features icc,legacy-crypto,rendering,signatures`. Compiles fresh from their local source. Their Rust code changes are immediately testable.

### CI runs on a PR

Same as contributor — the checkout includes `submodules: recursive`, so the vendor directory exists, so CI compiles from source. Always testing against the actual code in the PR, never stale binaries.

---

## The flow

```
feature branch ──PR──► dev
                        ↓ ci.yml auto-runs (analyze + macOS test)
                        ↓ repo owner reviews
                        ↓ optionally triggers Full Test (5 platforms)
                        ↓ merge

dev ──PR──► main (version bump in pubspec.yaml + CHANGELOG.md)
             ↓ ci.yml auto-runs
             ↓ repo owner triggers Full Test
             ↓ merge
             ↓
             release.yml auto-fires (detects pubspec.yaml changed)
               reads version from pubspec.yaml
               cross-compiles 13 native targets on 3 runners
               tests on 5 platforms
               creates git tag
               creates GitHub Release with pre-built binaries
               pub.dev dry-run
             ↓
             repo owner triggers publish.yml → pub.dev
```

---

## Four workflows

| Workflow | Trigger | Who can trigger | What | Cost |
|---|---|---|---|---|
| `ci.yml` | Every push/PR | Automatic | Analyze + macOS test (compile from source) | ~5 min |
| `full-test.yml` | Manual (Actions tab) | Repo owner only | 5 platforms: macOS, Linux, Windows, Android emulator, iOS simulator | ~20 min |
| `release.yml` | Push to main that changes pubspec.yaml | Automatic | Compile 13 targets, test 5 platforms, tag, GitHub Release with binaries | ~30 min |
| `publish.yml` | Manual (Actions tab) | Repo owner + reviewer | Push to pub.dev via OIDC | ~2 min |

---

## What the GitHub Release contains

13 pre-built native binaries, one per platform-architecture:

| Asset name | Platform |
|---|---|
| `macos-arm64-libpdf_oxide.dylib` | macOS Apple Silicon |
| `macos-x64-libpdf_oxide.dylib` | macOS Intel |
| `ios-arm64-libpdf_oxide.a` | iOS device |
| `ios-sim-arm64-libpdf_oxide.a` | iOS Simulator (Apple Silicon) |
| `ios-sim-x64-libpdf_oxide.a` | iOS Simulator (Intel) |
| `android-arm64-libpdf_oxide.so` | Android arm64 |
| `android-arm-libpdf_oxide.so` | Android arm |
| `android-x64-libpdf_oxide.so` | Android x64 |
| `android-x86-libpdf_oxide.so` | Android x86 |
| `linux-x64-libpdf_oxide.so` | Linux x64 |
| `linux-arm64-libpdf_oxide.so` | Linux arm64 |
| `windows-x64-pdf_oxide.dll` | Windows x64 |

These exist for consumers who install from pub.dev. Contributors and CI never download them.

---

## Five test platforms

| Platform | Runner | Method | Tests |
|---|---|---|---|
| macOS | macos-14 | `dart test` | 302 full suite |
| Linux | ubuntu-latest | `dart test` | 302 full suite |
| Windows | windows-latest | `dart test` | 302 full suite |
| Android | ubuntu-latest + emulator | Flutter integration test | 11 smoke tests |
| iOS | macos-14 + simulator | Flutter integration test | 11 smoke tests |

Desktop runs the full `dart test` suite. Mobile runs integration smoke tests in `example/integration_test/` via Flutter's test runner on emulator/simulator. `dart test` cannot run on mobile — the Dart VM doesn't exist there.

---

## Tested locally

| Scenario | How tested | Result |
|---|---|---|
| Contributor path (vendor exists, Rust installed) | `dart test` with submodule present | Compiles from source, 302 tests pass |
| Consumer path (no vendor) | Hid `Cargo.toml`, ran `dart test` | Downloads from GitHub Release URL (fails gracefully if no release exists yet) |
| Cache hit (second run) | Ran `dart test` again | Instant — cached binary reused |

---

## Cost optimization

| Technique | Effect |
|---|---|
| `Swatinem/rust-cache` | Caches `vendor/pdf_oxide/target/`. ~15 min → ~2 min |
| ci.yml = macOS only | Cheap automatic gate |
| full-test.yml = manual | Repo owner controls expensive runs |
| External contributors can't trigger full-test | `workflow_dispatch` requires write access |
| release.yml skips if tag exists | Non-version pubspec edits don't trigger release |

---

## GitHub Settings to configure

**Branch protection** (Settings → Branches):

| Branch | Rules |
|---|---|
| `main` | Require PR, require CI checks, no force push |
| `dev` | Require CI checks, no force push |

**Environments** (Settings → Environments):

| Environment | Protection |
|---|---|
| `publish` | Required reviewers |

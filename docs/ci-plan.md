# CI Architecture Plan

## Principles

1. **Makefile is the interface.** CI runs `make <target>`. All build logic lives in Makefile and scripts. CI YAML has zero build logic.

2. **Scripts handle their own deps.** Anything that might be missing on a dev machine (Rust targets, wasm-bindgen, binaryen, cross-compilers) is auto-installed by the script that needs it. Scripts detect CI via `$CI` env var — auto-install on CI, error with instructions on dev machines. CI never installs project deps.

3. **CI only bootstraps the toolchain.** FVM, Rust, Java — things that can't be installed from inside a project script because the script needs them to exist first. These live in CI setup actions.

4. **CI optimizes with caches.** Gradle, CocoaPods, Xcode derived data, WASM build output — CI-only performance optimizations. Never needed on dev machines. Live in CI setup actions.

5. **CI provides devices.** Android emulator, iOS simulator, xvfb, ChromeDriver — CI-only concerns for running device/browser tests on headless runners.

6. **One matrix, one action.** Every CI job is a cell in a matrix: `(runner, target, make-target, extras)`. One job definition, N entries. One orchestrator action (`flutter-target`) wires setup to make target.

---

## Dependency ownership

### Scripts (.sh / Makefile) — any machine, CI or dev

| Dep | Where | How | CI behavior | Dev behavior |
|---|---|---|---|---|
| Rust exists | compile_rust.sh top-level check | `command -v cargo` | setup-rust already installed it | Error: "Install Rust: https://rustup.rs" |
| build.json reads | compile_rust.sh, release.sh via pure bash `sed`/`grep` | Zero deps | Works | Works |
| Rust targets | compile_rust.sh `ensure_target` | `rustup target add` | Auto-install | Auto-install (user-space, safe) |
| wasm-bindgen-cli | compile_rust.sh `do_wasm` | `cargo install` | Auto-install | Auto-install (user-space, safe) |
| binaryen (wasm-opt) | compile_rust.sh `do_wasm` | package manager | CI: auto-install | Error with install command |
| gcc-aarch64 cross | compile_rust.sh `do_linux` | `apt-get` | CI: auto-install | Error with install command |
| GTK + ninja | Makefile linux targets | `apt-get` | CI: auto-install | Error with install command |

**Rules:**
- `rustup target add` and `cargo install` are always safe (user-space, no sudo). Auto-install everywhere.
- System packages (`apt-get`, `brew`, `choco`) auto-install only on CI (`$CI`), error with instructions on dev.
- Toolchain existence (`cargo`) is always a hard error — never auto-install.
- **Dart is never called from bash scripts.** Bash reads `build.json` with pure `sed`/`grep`. The dev's Dart might be behind `fvm dart`, bare `dart`, or not on PATH at all. Bash scripts have zero Dart/Flutter/FVM dependency.
- Dart code (`hook/build.dart`, `bin/setup.dart`) reads `build.json` via `dart:convert`. Two independent readers, same file, no coupling.

### CI actions — only on fresh CI runners

| Dep | Action | Why CI-only |
|---|---|---|
| FVM + Flutter SDK | `setup-fvm` | Can't install from inside a project — FVM must exist to run `fvm dart` |
| Rust + sccache | `setup-rust` | Can't install from inside a project — rustup must exist to run `cargo` |
| Java JDK | `setup-android` | Android dev machines have it. CI runners don't |
| Gradle cache | `setup-android` | CI optimization |
| CocoaPods cache | `setup-ios` | CI optimization |
| Xcode build cache | `setup-ios`, `setup-macos` | CI optimization |
| WASM build cache | `setup-web` | CI optimization |
| Android emulator + KVM | `setup-android` | CI device |
| iOS simulator | `setup-ios` | CI device |
| xvfb | `setup-linux` (inline in flutter-target) | CI headless display |
| ChromeDriver | `setup-web` | CI browser testing |

---

## File ownership — who does what

### tool/compile_rust.sh

Compiles Rust for any platform. Owns ALL compile-time deps:
- Checks `cargo` exists (hard error if missing)
- Reads `build.json` via pure bash `sed`/`grep` (zero external deps)
- `ensure_target` before every `cargo build` (auto-install, safe)
- `wasm-bindgen-cli` auto-install via `cargo install` (safe)
- `binaryen` CI-guard install (system package)
- `gcc-aarch64` CI-guard install (Linux cross-compile)
- Reads features from `build.json` — single source of truth
- Called by: Makefile (`build-wasm`, `compile-natives`), `hook/build.dart` (`_compileNativeFromCli`, `_compileWasm`)

### hook/build.dart

Flutter build hook. Owns native binary resolution + registration:
- `_compileNativeFromHook` — calls `cargo build` directly with NDK/Xcode env from Flutter's `BuildInput`
- `ensure_target` before `cargo build` (auto-install, safe)
- `_loadBuildConfig` — reads `build.json` via Dart `jsonDecode`
- `resolveWeb()` — public API for `setup.dart`, resolves WASM + JS
- Reads `assetHashes` for hash verification
- Called by: Flutter build system (`main`), `bin/setup.dart` (`resolveWeb`)

### hook/link.dart

Flutter link hook. Passthrough today. Foundation for tree-shaking.
- Called by: Flutter build system on release/AOT builds

### bin/setup.dart

CLI for manual asset resolution:
- `setup web` → calls `build.dart resolveWeb()`
- `setup <native-target>` → calls `flutter build <target> --debug`
- `setup --force <target>` → web: skip hash check. native: `flutter clean` first
- Reads `packageRoot` via `package_config`
- Called by: developers and CI Makefile targets

### tool/release.sh

Release pipeline logic. 7 modes:
- Reads `build.json` for repo, features, web assets
- Checks `python3` exists (hard error)
- `stamp_asset_hashes` — reads GitHub Release API + hashes local JS files
- Called by: `create-release.yml` workflow

### Makefile

The interface. Every target is self-contained:
- GTK check with CI-guard for Linux targets
- `setup_example_web` macro for web test setup
- Combined targets: `test-pkg-native`, `test-release`, `test`, `check`
- Called by: developers locally, CI via `flutter-target` action

### build.json

Single source of truth for constants shared across Dart and bash:
- `crate`, `repo`, `features.native`, `features.wasm`
- `web` asset map, `wasmBuildOutputs`
- Read by: `hook/build.dart`, `compile_rust.sh`, `release.sh`

---

## Actions (8 total)

### Foundation (2)

```
setup-fvm       Install FVM + Flutter SDK from .fvmrc
setup-rust      Install Rust toolchain + sccache + cargo caches
```

### Per-target setup (6)

```
setup-android   Java + Gradle cache + (emulator + KVM if requested)
setup-ios       Pods cache + Xcode cache + (simulator if requested)
setup-macos     Xcode cache
setup-linux     xvfb if headless requested
setup-windows   (empty — MSVC ships with Windows runners)
setup-web       ChromeDriver + WASM build cache
```

### Orchestrator (1)

```
flutter-target  setup-fvm → setup-rust → setup-<target> → make <target>
                Inputs: target, make-target, emulator, simulator, xvfb
                Zero platform logic — delegates everything to leaves
```

### Build (1)

```
compile         setup-rust → compile_rust.sh <platform>
                Script handles Rust targets, cross-deps, wasm tooling
```

---

## Matrix

### Dimensions

```
Runner:  ubuntu-latest | macos-14 | windows-latest
Target:  android | ios | macos | linux | windows | web
Task:    test-pkg-native | test-ops-web | test-example-* | test-release-*
```

### Valid combos

| Target | Valid runners | Notes |
|---|---|---|
| android | ubuntu, macos, windows | NDK available on all 3. .cmd fix for Windows |
| ios | macos | Needs Xcode |
| macos | macos | Needs macOS |
| linux | ubuntu | Needs Linux |
| windows | windows | Needs MSVC |
| web | ubuntu, macos, windows | WASM compiles on all 3 |

### Full test matrix (ready-to-test label)

| # | Runner | Target | Make target | Extras |
|---|---|---|---|---|
| 1 | macos-14 | macos | test-pkg-native | |
| 2 | ubuntu-latest | linux | test-pkg-native | |
| 3 | windows-latest | windows | test-pkg-native | |
| 4 | ubuntu-latest | web | test-ops-web | |
| 5 | macos-14 | macos | test-example-macos | |
| 6 | ubuntu-latest | linux | test-example-linux | xvfb |
| 7 | windows-latest | windows | test-example-windows | |
| 8 | ubuntu-latest | android | test-example-android | emulator |
| 9 | macos-14 | ios | test-example-ios | simulator |
| 10 | ubuntu-latest | web | test-example-web | |
| 11 | ubuntu-latest | android | test-release-android | |
| 12 | macos-14 | android | test-release-android | |
| 13 | windows-latest | android | test-release-android | |
| 14 | macos-14 | ios | test-release-ios | |
| 15 | macos-14 | macos | test-release-macos | |
| 16 | ubuntu-latest | linux | test-release-linux | |
| 17 | windows-latest | windows | test-release-windows | |
| 18 | ubuntu-latest | web | test-release-web | |
| 19 | macos-14 | web | test-release-web | |
| 20 | windows-latest | web | test-release-web | |

### CI workflow (every PR)

| # | Runner | Target | Make target |
|---|---|---|---|
| 1 | ubuntu-latest | — | analyze |
| 2 | macos-14 | macos | test-pkg-native |

### Compile matrix (release pipeline)

| # | Runner | Platform |
|---|---|---|
| 1 | macos-14 | macos |
| 2 | macos-14 | ios |
| 3 | ubuntu-latest | android |
| 4 | ubuntu-latest | linux |
| 5 | windows-latest | windows |
| 6 | ubuntu-latest | wasm |

---

## Workflow files (4)

```
ci.yml              Every PR: analyze + pkg-native test (fast gate)
full-test.yml       ready-to-test label: full matrix (20 jobs)
create-release.yml  Changelog push: compile + upload + publish
pr-lint.yml         PR title + promotion chain checks
```

Plus:
```
triage.yml          Auto-assign, auto-label, revoke stale ready-to-test
flutter-upgrade.yml Auto-detect new Flutter stable
debug-ssh.yml       SSH tunnel for CI debugging
```

---

## Makefile targets

### Dev

```
check                   analyze + test + test-example
analyze                 Dart + Rust static analysis
build                   build-native + build-wasm
test                    test-unit + test-ops
test-pkg-native         test-unit + test-ops-native
test-ops-web            all 3 web modes
test-example            test-example-macos + test-example-web
test-example-<target>   per-platform integration test
test-release            all 6 release-build tests
test-release-<target>   per-platform release build
```

### Release

```
compile-natives         compile Rust for all native targets
compile-wasm            compile Rust to WASM
```

---

## Edge cases and failure modes

### Dev runs `make test-release-android` without Rust
1. Makefile calls `flutter build apk --release`
2. Flutter triggers `hook/build.dart`
3. Hook tries download → fails (version 0.0.0)
4. Hook tries compile → `cargo` not found
5. Resolver throws: "Install Rust: https://rustup.rs"

### Dev runs `make test-release-web` without Rust
1. Makefile calls `setup_example_web` → `flutter pub run pdf_manipulator:setup`
2. `setup.dart` calls `resolveWeb()` → tries download → fails (0.0.0)
3. Falls through to compile → `compile_rust.sh --wasm`
4. Script checks `cargo` → not found → hard error with install URL

### Dev runs `make test-release-web` without binaryen
1. `compile_rust.sh` compiles WASM successfully
2. Hits `wasm-opt` step → `command -v wasm-opt` fails
3. Not CI → error with `brew install binaryen` / `apt-get` / `choco`

### Dev runs `make test-release-linux` without GTK
1. Makefile checks `pkg-config --exists gtk+-3.0` → fails
2. Not CI → error: "Run: sudo apt-get install -y ninja-build libgtk-3-dev"

### CI runner runs any target
1. `flutter-target` calls `setup-fvm` + `setup-rust` → toolchain ready
2. `flutter-target` calls `setup-<target>` → platform deps ready
3. `make <target>` → scripts auto-install project deps via CI guard
4. Everything works. No manual steps.

### Dev formats their machine
1. Install Rust: `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
2. Install FVM: `dart pub global activate fvm` (or: `make check DART=dart FLUTTER=flutter`)
3. Clone with `--recursive`
4. `make check` — scripts handle everything else

### No external deps for JSON reads
Bash scripts read `build.json` with pure bash (`sed`/`grep`). No
python3, no jq, no dart. The JSON is flat and we control its shape.

Shared helper function in each script that needs it:
```bash
_json_get() {
  sed -n "s/.*\"$1\": *\"\([^\"]*\)\".*/\1/p" "$PKG_ROOT/build.json"
}
```

For arrays and maps, `grep` + `sed` + `while read` — simple because
`build.json` is one key-value pair per line, never nested objects.

---

## The one-line summary

> Scripts own deps. CI owns toolchains + devices + caches. Makefile is the interface. One matrix, one orchestrator, one line per combo.

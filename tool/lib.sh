# shellcheck shell=bash
# Shared helpers — ONLY functions used by 2+ scripts belong here.
# Single-use functions stay in their own script. Source, don't execute.
#
#   source "$(dirname "$0")/lib.sh"
#
# Requires: PKG_ROOT set by the caller (defaults to ".").

# Ensure a Rust target is installed. Adds it if missing.
# rustup is the user-space toolchain manager (like fvm), so it self-manages
# both locally and on CI. Standalone tools do NOT — they go through
# provide_tool below.
ensure_target() {
  if ! rustup target list --installed | grep -qxF "$1"; then
    echo "  Installing Rust target: $1"
    rustup target add "$1"
  fi
}

# The single install gate for every standalone tool that is NOT a toolchain
# manager (binaryen, wasm-bindgen, cross-compilers). The caller does its own
# presence/version check, then calls this only when the tool is missing:
#   on CI   → run <installer>
#   locally → print <instructions> (one per line), exit 1, never auto-install
# Route every such tool through here; don't re-hand-roll the CI-vs-local check
# per site. Pass installer args via a one-shot env prefix, e.g.
#   WB_VERSION="$v" provide_tool _install_wasm_bindgen "Run: cargo install ..."
provide_tool() {
  local installer="$1"; shift
  if [ -n "${CI:-}" ]; then
    "$installer"
    return
  fi
  echo "Error: required tool not found. Install it:" >&2
  local line
  for line in "$@"; do echo "  $line" >&2; done
  exit 1
}

# Gate for tools that ship on every GitHub runner but aren't cleanly
# auto-installable (gh, jq). Present → use it. Missing → error with
# instructions and exit 1: on CI that means a broken runner image; locally
# the dev installs it. No auto-install path — unlike provide_tool, which is
# for tools CI genuinely has to fetch.
require_present() {
  local cmd="$1"; shift
  command -v "$cmd" >/dev/null 2>&1 && return 0
  echo "Error: '$cmd' not found. Install it:" >&2
  local line
  for line in "$@"; do echo "  $line" >&2; done
  exit 1
}

# jq is one of those pre-installed tools.
ensure_jq() {
  require_present jq \
    "macOS:   brew install jq" \
    "Linux:   sudo apt-get install jq" \
    "Windows: choco install jq"
}

# Read a value from a JSON file by jq path (defaults to build.json). jq does
# the parse, so nested paths work: json_get '.features.native'. Errors if the
# path is absent or null.
#   crate=$(json_get '.crate')
#   feat=$(json_get '.features.native')
json_get() {
  local expr="$1" file="${2:-${PKG_ROOT:-.}/build.json}"
  ensure_jq
  jq -er "$expr" "$file" 2>/dev/null || {
    echo "Error: '$expr' not found in $file" >&2
    exit 1
  }
}

# sha256 of a file. GNU coreutils ships sha256sum; macOS ships
# `shasum -a 256`. Use whichever exists; print the bare hash. Feed the file on
# stdin, never as a path argument: both escape the output line — a backslash
# before the digest — when the filename contains a backslash, as every Windows
# Git Bash path does. Stdin keeps the filename out of the output entirely.
sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum < "$1" | awk '{print $1}'
  else
    shasum -a 256 < "$1" | awk '{print $1}'
  fi
}

# Highest version-named subdirectory of $1 (e.g. an Android NDK dir), or
# nothing. Portable version-sort (older BSD sort lacks the V flag): sort the
# basenames by dotted numeric fields. Prints the full path.
latest_version_subdir() {
  local base="$1" name
  name=$(ls "$base" 2>/dev/null | sort -t. -k1,1n -k2,2n -k3,3n | tail -1)
  [ -n "$name" ] && printf '%s\n' "$base/$name"
}

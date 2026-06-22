#!/usr/bin/env bash
set -euo pipefail
#
# Upgrade radar — the ONE place that watches every pinned version this repo
# owns and can bump automatically. upgrade-check.yml runs `apply` daily and
# opens a single reviewed PR with whatever drifted.
#
# Watched here (clean upstream, real churn):
#   Flutter SDK   .fvmrc + example/.fvmrc        Google release channel
#   fvm tool      FVM_VERSION + 4 sha256         leoafarias/fvm
#   Chrome        CHROME_VERSION + 6 sha256      chrome-for-testing (Stable)
#   binaryen      BINARYEN_VERSION + 3 sha256    WebAssembly/binaryen
#   bore          BORE_VERSION + 3 sha256        ekzhang/bore
#   zizmor gate   ZIZMOR_VERSION                 PyPI
#   actionlint    ACTIONLINT_VERSION             rhysd/actionlint
#
# Owned elsewhere by design (NOT here):
#   pub deps, GitHub-action SHAs   Dependabot (.github/dependabot.yml)
#   build.json (baseTag, crate,    manual — pdf_oxide source facts; move only
#     features, outputs)             when the vendored submodule does
#
# JSON is parsed with jq — gh's embedded `--jq` for gh-api responses,
# ensure_jq + jq for curl'd manifests. In-place version bumps stay targeted
# sed (to preserve each file's formatting). No python, no dart.
#
# Usage:  tool/ci/upgrade.sh check   # report drift, write nothing
#         tool/ci/upgrade.sh apply   # rewrite the pinned files in place

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"  # tool/ci/ → repo root
VERSIONS="$ROOT/tool/versions.env"

# shellcheck source=/dev/null  # runtime path; not followed at lint time
source "$VERSIONS"
source "$ROOT/tool/lib.sh"

MODE="${1:-check}"
case "$MODE" in
  check|apply) ;;
  *) echo "usage: tool/ci/upgrade.sh [check|apply]" >&2; exit 2 ;;
esac

ensure_jq

drift=0

gh_latest_tag() {  # owner/repo -> latest release tag, verbatim
  gh api "repos/$1/releases/latest" --jq '.tag_name' 2>/dev/null || true
}

# sha256 of a downloadable asset, or empty on failure. Downloads to a file
# (never a shell var) so binary content survives intact and a 404 can't
# masquerade as the empty-string hash.
sha256_of() {  # url
  local tmp; tmp="$(mktemp)"
  if curl -fsSL "$1" -o "$tmp" 2>/dev/null && [ -s "$tmp" ]; then
    sha256sum "$tmp" | cut -d' ' -f1
  fi
  rm -f "$tmp"
}

set_kv() {  # KEY value file — replace KEY="old" with KEY="new" in place
  sed -i.bak -E "s|^$1=\"[^\"]*\"|$1=\"$2\"|" "$3" && rm -f "$3.bak"
}

# ── Flutter SDK (.fvmrc + example/.fvmrc) ────────────────────────────
flutter_cur="$(json_get '.flutter' "$ROOT/.fvmrc")"
releases="$(curl -fsSL https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json 2>/dev/null || true)"
stable_hash="$(echo "$releases" | jq -r '.current_release.stable // empty' 2>/dev/null || true)"
flutter_latest="$(echo "$releases" | jq -r --arg h "$stable_hash" '.releases[] | select(.hash == $h) | .version // empty' 2>/dev/null | head -1 || true)"
if [ -n "$flutter_latest" ] && [ -n "$flutter_cur" ] && [ "$flutter_cur" != "$flutter_latest" ]; then
  drift=1
  echo "flutter: $flutter_cur -> $flutter_latest"
  if [ "$MODE" = apply ]; then
    for f in "$ROOT/.fvmrc" "$ROOT/example/.fvmrc"; do
      sed -i.bak "s/\"flutter\": *\"[^\"]*\"/\"flutter\": \"$flutter_latest\"/" "$f" && rm -f "$f.bak"
    done
  fi
fi

# ── fvm tool (version + 4 verified sha256, all in versions.env) ──────
fvm_latest="$(gh_latest_tag leoafarias/fvm | sed 's/^v//')"
if [ -n "$fvm_latest" ] && [ "$fvm_latest" != "$FVM_VERSION" ]; then
  if [ "$MODE" = apply ]; then
    u="https://github.com/leoafarias/fvm/releases/download/$fvm_latest"
    lx="$(sha256_of "$u/fvm-$fvm_latest-linux-x64.tar.gz")"
    ma="$(sha256_of "$u/fvm-$fvm_latest-macos-arm64.tar.gz")"
    mx="$(sha256_of "$u/fvm-$fvm_latest-macos-x64.tar.gz")"
    win="$(sha256_of "$u/fvm-$fvm_latest-windows-x64.zip")"
    if [ -n "$lx" ] && [ -n "$ma" ] && [ -n "$mx" ] && [ -n "$win" ]; then
      drift=1; echo "fvm: $FVM_VERSION -> $fvm_latest (+ 4 sha256)"
      set_kv FVM_VERSION "$fvm_latest" "$VERSIONS"
      set_kv FVM_SHA256_LINUX_X64 "$lx" "$VERSIONS"
      set_kv FVM_SHA256_MACOS_ARM64 "$ma" "$VERSIONS"
      set_kv FVM_SHA256_MACOS_X64 "$mx" "$VERSIONS"
      set_kv FVM_SHA256_WINDOWS_X64 "$win" "$VERSIONS"
    else
      echo "fvm: $fvm_latest available but an asset download failed — bump by hand"
    fi
  else
    drift=1; echo "fvm: $FVM_VERSION -> $fvm_latest (apply fetches + verifies 4 sha256)"
  fi
fi

# ── zizmor gate (ZIZMOR_VERSION in versions.env; run in pr-lint.yml) ──
ziz_latest="$(curl -fsSL https://pypi.org/pypi/zizmor/json 2>/dev/null | jq -r '.info.version // empty' 2>/dev/null || true)"
if [ -n "$ZIZMOR_VERSION" ] && [ -n "$ziz_latest" ] && [ "$ZIZMOR_VERSION" != "$ziz_latest" ]; then
  drift=1
  echo "zizmor: $ZIZMOR_VERSION -> $ziz_latest"
  [ "$MODE" = apply ] && set_kv ZIZMOR_VERSION "$ziz_latest" "$VERSIONS"
fi

# ── actionlint (the other pr-lint gate; version in versions.env) ─────
al_latest="$(gh_latest_tag rhysd/actionlint | sed 's/^v//')"
if [ -n "$al_latest" ] && [ "$al_latest" != "$ACTIONLINT_VERSION" ]; then
  drift=1
  echo "actionlint: $ACTIONLINT_VERSION -> $al_latest"
  [ "$MODE" = apply ] && set_kv ACTIONLINT_VERSION "$al_latest" "$VERSIONS"
fi

# ── binaryen (version + 3 verified sha256, all in versions.env) ──────
# Detection is a cheap tag compare; the downloads run only on apply, and any
# single 404 aborts the whole bump rather than writing a half-correct pin.
bin_latest="$(gh_latest_tag WebAssembly/binaryen)"
if [ -n "$bin_latest" ] && [ "$bin_latest" != "$BINARYEN_VERSION" ]; then
  if [ "$MODE" = apply ]; then
    u="https://github.com/WebAssembly/binaryen/releases/download/$bin_latest"
    lx="$(sha256_of "$u/binaryen-$bin_latest-x86_64-linux.tar.gz")"
    mac="$(sha256_of "$u/binaryen-$bin_latest-arm64-macos.tar.gz")"
    win="$(sha256_of "$u/binaryen-$bin_latest-x86_64-windows.tar.gz")"
    if [ -n "$lx" ] && [ -n "$mac" ] && [ -n "$win" ]; then
      drift=1; echo "binaryen: $BINARYEN_VERSION -> $bin_latest (+ 3 sha256)"
      set_kv BINARYEN_VERSION "$bin_latest" "$VERSIONS"
      set_kv BINARYEN_SHA256_LINUX_X64 "$lx" "$VERSIONS"
      set_kv BINARYEN_SHA256_MACOS_ARM64 "$mac" "$VERSIONS"
      set_kv BINARYEN_SHA256_WINDOWS_X64 "$win" "$VERSIONS"
    else
      echo "binaryen: $bin_latest available but an asset download failed — bump by hand"
    fi
  else
    drift=1; echo "binaryen: $BINARYEN_VERSION -> $bin_latest (apply fetches + verifies 3 sha256)"
  fi
fi

# ── bore (version + 3 verified sha256, all in versions.env) ──────────
bore_latest="$(gh_latest_tag ekzhang/bore)"
if [ -n "$bore_latest" ] && [ "$bore_latest" != "$BORE_VERSION" ]; then
  if [ "$MODE" = apply ]; then
    u="https://github.com/ekzhang/bore/releases/download/$bore_latest"
    lx="$(sha256_of "$u/bore-$bore_latest-x86_64-unknown-linux-musl.tar.gz")"
    mac="$(sha256_of "$u/bore-$bore_latest-aarch64-apple-darwin.tar.gz")"
    win="$(sha256_of "$u/bore-$bore_latest-x86_64-pc-windows-msvc.zip")"
    if [ -n "$lx" ] && [ -n "$mac" ] && [ -n "$win" ]; then
      drift=1; echo "bore: $BORE_VERSION -> $bore_latest (+ 3 sha256)"
      set_kv BORE_VERSION "$bore_latest" "$VERSIONS"
      set_kv BORE_SHA256_LINUX_X64 "$lx" "$VERSIONS"
      set_kv BORE_SHA256_MACOS_ARM64 "$mac" "$VERSIONS"
      set_kv BORE_SHA256_WINDOWS_X64 "$win" "$VERSIONS"
    else
      echo "bore: $bore_latest available but an asset download failed — bump by hand"
    fi
  else
    drift=1; echo "bore: $BORE_VERSION -> $bore_latest (apply fetches + verifies 3 sha256)"
  fi
fi

# ── Chrome for Testing (version + 6 verified sha256, all in versions.env) ──
# chrome-for-testing publishes no digests, so each bump re-downloads and
# self-hashes all 6 assets. The CDN prunes old versions, so this must keep
# the pin fresh or the chrome action's verified download 404s.
manifest="$(curl -fsSL https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json 2>/dev/null || true)"
chrome_latest="$(echo "$manifest" | jq -r '.channels.Stable.version // empty' 2>/dev/null || true)"
if [ -n "$chrome_latest" ] && [ "$chrome_latest" != "$CHROME_VERSION" ]; then
  if [ "$MODE" = apply ]; then
    u="https://storage.googleapis.com/chrome-for-testing-public/$chrome_latest"
    cl="$(sha256_of "$u/linux64/chrome-linux64.zip")";     dl="$(sha256_of "$u/linux64/chromedriver-linux64.zip")"
    cm="$(sha256_of "$u/mac-arm64/chrome-mac-arm64.zip")"; dm="$(sha256_of "$u/mac-arm64/chromedriver-mac-arm64.zip")"
    cw="$(sha256_of "$u/win64/chrome-win64.zip")";         dw="$(sha256_of "$u/win64/chromedriver-win64.zip")"
    if [ -n "$cl" ] && [ -n "$dl" ] && [ -n "$cm" ] && [ -n "$dm" ] && [ -n "$cw" ] && [ -n "$dw" ]; then
      drift=1; echo "chrome: $CHROME_VERSION -> $chrome_latest (+ 6 sha256)"
      set_kv CHROME_VERSION "$chrome_latest" "$VERSIONS"
      set_kv CHROME_SHA256_LINUX64 "$cl" "$VERSIONS"
      set_kv CHROME_SHA256_MAC_ARM64 "$cm" "$VERSIONS"
      set_kv CHROME_SHA256_WIN64 "$cw" "$VERSIONS"
      set_kv CHROMEDRIVER_SHA256_LINUX64 "$dl" "$VERSIONS"
      set_kv CHROMEDRIVER_SHA256_MAC_ARM64 "$dm" "$VERSIONS"
      set_kv CHROMEDRIVER_SHA256_WIN64 "$dw" "$VERSIONS"
    else
      echo "chrome: $chrome_latest available but an asset download failed — bump by hand"
    fi
  else
    drift=1; echo "chrome: $CHROME_VERSION -> $chrome_latest (apply fetches + verifies 6 sha256)"
  fi
fi

[ "$drift" -eq 0 ] && echo "All watched pins are current."
exit 0

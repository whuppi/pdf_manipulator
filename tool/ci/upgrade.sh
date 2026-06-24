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
# Usage:  tool/ci/upgrade.sh check              # report drift, write nothing
#         tool/ci/upgrade.sh apply              # rewrite the pinned files in place
#         tool/ci/upgrade.sh verify-pinned      # re-hash the pins, flag a repoint
#         tool/ci/upgrade.sh check-availability # HEAD the pins, flag a prune

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"  # tool/ci/ → repo root
VERSIONS="$ROOT/tool/versions.env"

# shellcheck source=/dev/null  # runtime path; not followed at lint time
source "$VERSIONS"
source "$ROOT/tool/lib.sh"

MODE="${1:-check}"
case "$MODE" in
  check|apply|verify-pinned|check-availability) ;;
  *) echo "usage: tool/ci/upgrade.sh [check|apply|verify-pinned|check-availability]" >&2; exit 2 ;;
esac

ensure_jq

drift=0
blocked=0   # a bump whose asset fetch failed sets this; the run then exits
            # nonzero at the end instead of passing silently.

gh_latest_tag() {  # owner/repo -> latest release tag, verbatim
  gh api "repos/$1/releases/latest" --jq '.tag_name' 2>/dev/null || true
}

# Hardened GET for the fetches below: fail-closed, capped redirects, and a
# few retries so a transient blip doesn't fail the daily run.
_fetch() { curl -fsSL --retry 3 --retry-delay 2 --max-redirs 5 --connect-timeout 10 --max-time 30 "$@"; }

# sha256 of a downloadable asset, or empty on failure. Downloads to a file
# (never a shell var) so binary content survives intact and a 404 can't
# masquerade as the empty-string hash.
sha256_of() {  # url
  local tmp; tmp="$(mktemp)"
  if _fetch "$1" -o "$tmp" 2>/dev/null && [ -s "$tmp" ]; then
    sha256_file "$tmp"
  fi
  rm -f "$tmp"
}

set_kv() {  # KEY value file — replace the KEY="old" line with KEY="new"
  # versions.env is sourced, so a value is executed on read. Validate the shape
  # before writing (version or 64-hex sha) so a malformed upstream string can't
  # be persisted and run.
  case "$1" in
    *SHA256*)  printf '%s' "$2" | grep -qE '^[0-9a-f]{64}$' \
                 || { echo "set_kv: refusing non-sha256 for $1: '$2'" >&2; return 1; } ;;
    *VERSION*) printf '%s' "$2" | grep -qE '^[A-Za-z0-9._+-]+$' \
                 || { echo "set_kv: refusing malformed version for $1: '$2'" >&2; return 1; } ;;
    *)         echo "set_kv: unknown key shape '$1' (expect *_VERSION or *_SHA256_*)" >&2; return 1 ;;
  esac
  # The value travels via the environment — not a sed replacement, not awk -v —
  # so a |, &, or backslash in it stays literal data (sed's replacement string
  # and awk's -v both interpret those). tmp+mv leaves the file intact if awk
  # ever fails mid-write.
  local tmp="$3.tmp"
  if sk_key="$1" sk_val="$2" awk '
        BEGIN { k = ENVIRON["sk_key"]; v = ENVIRON["sk_val"] }
        $0 ~ "^" k "=" { print k "=\"" v "\""; next }
        { print }
      ' "$3" > "$tmp"; then
    mv "$tmp" "$3"
  else
    rm -f "$tmp"
    return 1
  fi
}

# Every pinned asset as tool<TAB>platform<TAB>url<TAB>sha256. Single source for
# the checks below; mirrors the bump blocks, so keep the two in sync.
asset_urls() {
  local fu="https://github.com/leoafarias/fvm/releases/download/$FVM_VERSION"
  printf 'fvm\tlinux-x64\t%s\t%s\n'   "$fu/fvm-$FVM_VERSION-linux-x64.tar.gz"   "$FVM_SHA256_LINUX_X64"
  printf 'fvm\tmacos-arm64\t%s\t%s\n' "$fu/fvm-$FVM_VERSION-macos-arm64.tar.gz" "$FVM_SHA256_MACOS_ARM64"
  printf 'fvm\tmacos-x64\t%s\t%s\n'   "$fu/fvm-$FVM_VERSION-macos-x64.tar.gz"   "$FVM_SHA256_MACOS_X64"
  printf 'fvm\twindows-x64\t%s\t%s\n' "$fu/fvm-$FVM_VERSION-windows-x64.zip"    "$FVM_SHA256_WINDOWS_X64"
  local bu="https://github.com/WebAssembly/binaryen/releases/download/$BINARYEN_VERSION"
  printf 'binaryen\tlinux-x64\t%s\t%s\n'   "$bu/binaryen-$BINARYEN_VERSION-x86_64-linux.tar.gz"   "$BINARYEN_SHA256_LINUX_X64"
  printf 'binaryen\tmacos-arm64\t%s\t%s\n' "$bu/binaryen-$BINARYEN_VERSION-arm64-macos.tar.gz"    "$BINARYEN_SHA256_MACOS_ARM64"
  printf 'binaryen\twindows-x64\t%s\t%s\n' "$bu/binaryen-$BINARYEN_VERSION-x86_64-windows.tar.gz" "$BINARYEN_SHA256_WINDOWS_X64"
  local ru="https://github.com/ekzhang/bore/releases/download/$BORE_VERSION"
  printf 'bore\tlinux-x64\t%s\t%s\n'   "$ru/bore-$BORE_VERSION-x86_64-unknown-linux-musl.tar.gz" "$BORE_SHA256_LINUX_X64"
  printf 'bore\tmacos-arm64\t%s\t%s\n' "$ru/bore-$BORE_VERSION-aarch64-apple-darwin.tar.gz"      "$BORE_SHA256_MACOS_ARM64"
  printf 'bore\twindows-x64\t%s\t%s\n' "$ru/bore-$BORE_VERSION-x86_64-pc-windows-msvc.zip"       "$BORE_SHA256_WINDOWS_X64"
  local cu="https://storage.googleapis.com/chrome-for-testing-public/$CHROME_VERSION"
  printf 'chrome\tlinux-x64\t%s\t%s\n'   "$cu/linux64/chrome-linux64.zip"     "$CHROME_SHA256_LINUX_X64"
  printf 'chrome\tmacos-arm64\t%s\t%s\n' "$cu/mac-arm64/chrome-mac-arm64.zip" "$CHROME_SHA256_MACOS_ARM64"
  printf 'chrome\twindows-x64\t%s\t%s\n' "$cu/win64/chrome-win64.zip"         "$CHROME_SHA256_WINDOWS_X64"
  printf 'chromedriver\tlinux-x64\t%s\t%s\n'   "$cu/linux64/chromedriver-linux64.zip"     "$CHROMEDRIVER_SHA256_LINUX_X64"
  printf 'chromedriver\tmacos-arm64\t%s\t%s\n' "$cu/mac-arm64/chromedriver-mac-arm64.zip" "$CHROMEDRIVER_SHA256_MACOS_ARM64"
  printf 'chromedriver\twindows-x64\t%s\t%s\n' "$cu/win64/chromedriver-win64.zip"         "$CHROMEDRIVER_SHA256_WINDOWS_X64"
}

# HTTP status of a URL, following GitHub's asset redirect, or 000 if unreachable.
# Always exits 0 so a caller's $(...) never trips set -e.
http_status() {  # url -> code
  local code
  code="$(curl -sS -o /dev/null -IL --connect-timeout 10 --retry 2 --retry-delay 2 \
          --max-redirs 5 -w '%{http_code}' "$1" 2>/dev/null || true)"
  printf '%s' "${code:-000}"
}

# Daily integrity: a 404/410 means the pin was pruned, a 200 with a changed hash
# means a same-version repoint. A transient code warns rather than failing.
verify_pinned() {
  local t=0 tool plat url want code got
  while IFS=$'\t' read -r tool plat url want; do
    code="$(http_status "$url")"
    case "$code" in
      404|410) echo "::error::verify-pinned: $tool $plat pruned upstream ($code), re-pin: $url" >&2; t=1; continue ;;
      200) ;;
      *) echo "::warning::verify-pinned: $tool $plat HEAD returned $code (transient?): $url" >&2; continue ;;
    esac
    got="$(sha256_of "$url")"
    if [ -z "$got" ]; then
      echo "::warning::verify-pinned: $tool $plat 200 but empty body (transient?): $url" >&2
    elif [ "$got" != "$want" ]; then
      echo "::error::verify-pinned: $tool $plat REPOINT, pinned $want now $got: $url" >&2; t=1
    fi
  done < <(asset_urls)
  [ "$t" -eq 0 ] && { echo "verify-pinned: all pinned assets present and matching."; return 0; }
  echo "::error::verify-pinned: a pinned asset is gone or changed; do NOT bump, investigate" >&2
  return 1
}

# PR hot-path existence check, HEAD only (no download or hash). Fails on a
# definitive 404/410, warns on transient. Runs on PR activity, so a disabled
# daily cron can't hide a pruned pin until the build breaks.
check_availability() {
  local bad=0 tool plat url _sha code
  while IFS=$'\t' read -r tool plat url _sha; do
    code="$(http_status "$url")"
    case "$code" in
      200) ;;
      404|410) echo "::error::pin unavailable: $tool $plat gone upstream ($code): $url" >&2; bad=1 ;;
      *) echo "::warning::pin check: $tool $plat HEAD returned $code (transient?): $url" >&2 ;;
    esac
  done < <(asset_urls)
  [ "$bad" -eq 0 ] && echo "check-availability: all pinned assets reachable."
  return "$bad"
}

# Each check runs alone, then exits.
if [ "$MODE" = verify-pinned ]; then verify_pinned && exit 0; exit 1; fi
if [ "$MODE" = check-availability ]; then check_availability && exit 0; exit 1; fi

# ── Flutter SDK (.fvmrc + example/.fvmrc) ────────────────────────────
flutter_cur="$(json_get '.flutter' "$ROOT/.fvmrc")"
releases="$(_fetch https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json 2>/dev/null || true)"
stable_hash="$(jq -r '.current_release.stable // empty' <<< "$releases" 2>/dev/null || true)"
flutter_latest="$(jq -r --arg h "$stable_hash" '.releases[] | select(.hash == $h) | .version // empty' <<< "$releases" 2>/dev/null | head -1 || true)"
# flutter_latest is interpolated into the sed below, bypassing set_kv. Keep the
# exact-semver gate or a crafted upstream value becomes sed injection.
if [ -n "$flutter_latest" ] && ! printf '%s' "$flutter_latest" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "::error::refuse: implausible flutter version from upstream: '$flutter_latest'" >&2
  exit 1
fi
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
      echo "::error::fvm: $fvm_latest available but an asset download failed — bump by hand"; blocked=1
    fi
  else
    drift=1; echo "fvm: $FVM_VERSION -> $fvm_latest (apply fetches + verifies 4 sha256)"
  fi
fi

# ── zizmor gate (ZIZMOR_VERSION in versions.env; run in pr-lint.yml) ──
ziz_latest="$(_fetch https://pypi.org/pypi/zizmor/json 2>/dev/null | jq -r '.info.version // empty' 2>/dev/null || true)"
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
      echo "::error::binaryen: $bin_latest available but an asset download failed — bump by hand"; blocked=1
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
      echo "::error::bore: $bore_latest available but an asset download failed — bump by hand"; blocked=1
    fi
  else
    drift=1; echo "bore: $BORE_VERSION -> $bore_latest (apply fetches + verifies 3 sha256)"
  fi
fi

# ── Chrome for Testing (version + 6 verified sha256, all in versions.env) ──
# chrome-for-testing publishes no digests, so each bump re-downloads and
# self-hashes all 6 assets. The CDN prunes old versions, so this must keep
# the pin fresh or the chrome action's verified download 404s.
manifest="$(_fetch https://googlechromelabs.github.io/chrome-for-testing/last-known-good-versions-with-downloads.json 2>/dev/null || true)"
chrome_latest="$(jq -r '.channels.Stable.version // empty' <<< "$manifest" 2>/dev/null || true)"
if [ -n "$chrome_latest" ] && [ "$chrome_latest" != "$CHROME_VERSION" ]; then
  if [ "$MODE" = apply ]; then
    u="https://storage.googleapis.com/chrome-for-testing-public/$chrome_latest"
    cl="$(sha256_of "$u/linux64/chrome-linux64.zip")";     dl="$(sha256_of "$u/linux64/chromedriver-linux64.zip")"
    cm="$(sha256_of "$u/mac-arm64/chrome-mac-arm64.zip")"; dm="$(sha256_of "$u/mac-arm64/chromedriver-mac-arm64.zip")"
    cw="$(sha256_of "$u/win64/chrome-win64.zip")";         dw="$(sha256_of "$u/win64/chromedriver-win64.zip")"
    if [ -n "$cl" ] && [ -n "$dl" ] && [ -n "$cm" ] && [ -n "$dm" ] && [ -n "$cw" ] && [ -n "$dw" ]; then
      drift=1; echo "chrome: $CHROME_VERSION -> $chrome_latest (+ 6 sha256)"
      set_kv CHROME_VERSION "$chrome_latest" "$VERSIONS"
      set_kv CHROME_SHA256_LINUX_X64 "$cl" "$VERSIONS"
      set_kv CHROME_SHA256_MACOS_ARM64 "$cm" "$VERSIONS"
      set_kv CHROME_SHA256_WINDOWS_X64 "$cw" "$VERSIONS"
      set_kv CHROMEDRIVER_SHA256_LINUX_X64 "$dl" "$VERSIONS"
      set_kv CHROMEDRIVER_SHA256_MACOS_ARM64 "$dm" "$VERSIONS"
      set_kv CHROMEDRIVER_SHA256_WINDOWS_X64 "$dw" "$VERSIONS"
    else
      echo "::error::chrome: $chrome_latest available but an asset download failed — bump by hand"; blocked=1
    fi
  else
    drift=1; echo "chrome: $CHROME_VERSION -> $chrome_latest (apply fetches + verifies 6 sha256)"
  fi
fi

[ "$drift" -eq 0 ] && echo "All watched pins are current."
if [ "$blocked" -ne 0 ]; then
  echo "::error::one or more pins are blocked (upstream had a new version but its asset fetch failed); fix by hand" >&2
  exit 1
fi
exit 0

# Shared helpers — ONLY functions used by 2+ scripts belong here.
# Single-use functions stay in their own script. Source, don't execute.
#
#   source "$(dirname "$0")/lib.sh"
#
# Requires: PKG_ROOT set by the caller (defaults to ".").

# Ensure a Rust target is installed. Adds it if missing.
ensure_target() {
  if ! rustup target list --installed | grep -qx "$1"; then
    echo "  Installing Rust target: $1"
    rustup target add "$1"
  fi
}

# Read a string value from build.json by key name.
# Errors if the key is not found.
_json_get() {
  local root="${PKG_ROOT:-.}"
  local val
  val=$(sed -n "s/.*\"$1\": *\"\([^\"]*\)\".*/\1/p" "$root/build.json")
  if [ -z "$val" ]; then
    echo "Error: key '$1' not found in build.json" >&2
    exit 1
  fi
  echo "$val"
}

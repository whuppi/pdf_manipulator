#!/bin/bash
# Bumps version in both Cargo.toml and pyproject.toml
# Usage: bump-version.sh <new-version>

set -euo pipefail

if [ -z "${1:-}" ]; then
  echo "Usage: bump-version.sh <new-version>" >&2
  echo "Example: bump-version.sh 0.3.6" >&2
  exit 1
fi

VERSION="$1"

sed -i "s/^version = \".*\"/version = \"${VERSION}\"/" Cargo.toml
sed -i "s/^version = \".*\"/version = \"${VERSION}\"/" pyproject.toml

echo "Bumped to ${VERSION} in Cargo.toml and pyproject.toml"

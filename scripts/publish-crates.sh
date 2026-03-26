#!/usr/bin/env bash
set -euo pipefail

# Manual crates.io publish script (prefer CI workflow for releases).
# Usage: ./scripts/publish-crates.sh

VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="edgeparse-core") | .version')
echo "Publishing pdf-cos @ $VERSION"
cargo publish -p pdf-cos

echo "Waiting 30s for crates.io index…"
sleep 30

echo "Publishing edgeparse-core @ $VERSION"
cargo publish -p edgeparse-core

echo "Waiting 30s for crates.io index…"
sleep 30

echo "Publishing edgeparse (CLI) @ $VERSION"
cargo publish -p edgeparse-cli --no-verify
echo "Done."

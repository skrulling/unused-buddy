#!/usr/bin/env bash
set -euo pipefail

# Local dry run helper: expects release assets and manifest/checksums in a directory.
# Usage: ./scripts/npm/pack-local.sh /path/to/release-assets v0.1.0

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <release-assets-dir> <tag>"
  exit 1
fi

RELEASE_ASSETS_DIR="$1"
RELEASE_TAG="$2"

if [ ! -d "$RELEASE_ASSETS_DIR" ]; then
  echo "release assets directory not found: $RELEASE_ASSETS_DIR"
  exit 1
fi

RELEASE_ASSETS_DIR="$RELEASE_ASSETS_DIR" RELEASE_TAG="$RELEASE_TAG" DRY_RUN=1 node scripts/npm/publish-from-release.mjs

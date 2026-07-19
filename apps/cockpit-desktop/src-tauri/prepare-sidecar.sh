#!/usr/bin/env bash
# Build the cockpit-runner and stage it as a Tauri sidecar binary.
#
# Tauri resolves `externalBin` entries by appending the host target triple, so
# the runner is copied to `binaries/cockpit-runner-<triple><ext>`. Run this
# before `npm run tauri:build` (or `tauri:dev`) to package the runner alongside
# the desktop app.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
BIN_DIR="$SCRIPT_DIR/binaries"

TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
if [ -z "$TRIPLE" ]; then
  echo "could not determine host target triple" >&2
  exit 1
fi

EXT=""
case "$TRIPLE" in
  *windows*) EXT=".exe" ;;
esac

echo "Building cockpit-runner and cockpit-evaluator (release) for $TRIPLE"
cargo build --release -p cockpit-runner -p cockpit-evaluator --features cockpit-runner/live-acp --manifest-path "$WORKSPACE_ROOT/Cargo.toml"

mkdir -p "$BIN_DIR"
for NAME in cockpit-runner cockpit-evaluator; do
  SRC="$WORKSPACE_ROOT/target/release/$NAME$EXT"
  DST="$BIN_DIR/$NAME-$TRIPLE$EXT"
  cp "$SRC" "$DST"
  # Tauri copies these files verbatim into the app bundle.
  chmod +x "$DST"
  echo "Staged sidecar: $DST"
done

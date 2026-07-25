#!/usr/bin/env bash
# Builds the app and actually starts it.
#
# `cargo test` compiles and passes on a binary that aborts the moment macOS
# hands it a window: everything that goes wrong at launch — a plugin never
# registered, a missing icon file, a panel converted too early — happens in
# `did_finish_launching`, after every test has already gone green. Nothing but
# running the thing catches that.
#
#   tools/smoke.sh          # release build, the one that ships
#   MODE=debug tools/smoke.sh
#
# Green means the process was still alive after ALIVE_SECS, and its log shows
# setup finished. Red prints whatever the app said on its way down.

set -euo pipefail

cd "$(dirname "$0")/.."

MODE="${MODE:-release}"
ALIVE_SECS="${ALIVE_SECS:-6}"
LOG="$HOME/Library/Application Support/nod/logs/debug.log"
OUT="$(mktemp -t nod-smoke)"

if [ "$MODE" = "release" ]; then
  cargo build --release --manifest-path src-tauri/Cargo.toml
  BIN=src-tauri/target/release/nod
else
  cargo build --manifest-path src-tauri/Cargo.toml
  BIN=src-tauri/target/debug/nod
fi

rm -f "$LOG"
"$BIN" >"$OUT" 2>&1 &
PID=$!
sleep "$ALIVE_SECS"

if ! kill -0 "$PID" 2>/dev/null; then
  echo "smoke: Nod died within ${ALIVE_SECS}s of starting"
  echo "--- what it printed:"
  cat "$OUT"
  exit 1
fi

kill "$PID" 2>/dev/null || true
wait "$PID" 2>/dev/null || true

if ! grep -q "setup complete" "$LOG" 2>/dev/null; then
  echo "smoke: Nod stayed up but never finished setup — log says:"
  cat "$LOG" 2>/dev/null || echo "(no log at all)"
  exit 1
fi

echo "smoke: alive for ${ALIVE_SECS}s, setup complete"

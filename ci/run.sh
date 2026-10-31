#!/usr/bin/env bash
# Runs a command and re-emits compiler/test failures as GitHub annotations,
# so they are readable through the check-runs API without downloading logs.
set -uo pipefail
out="$(mktemp)"
"$@" 2>&1 | tee "$out"
status=${PIPESTATUS[0]}
if [ "$status" -ne 0 ]; then
  sed 's/\x1b\[[0-9;]*m//g' "$out" | grep -E -A7 '^error|panicked at|^---- .* stdout ----|FAILED' | head -150 | tr -d '\r' | while IFS= read -r line; do
    printf '::error::%s\n' "${line:0:900}"
  done
fi
exit "$status"

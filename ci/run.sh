#!/usr/bin/env bash
# Runs a command and re-emits compiler/test failures as ONE GitHub annotation (GitHub caps the
# number of annotations per step), so they are readable via the check-runs API without logs.
set -uo pipefail
out="$(mktemp)"
"$@" 2>&1 | tee "$out"
status=${PIPESTATUS[0]}
if [ "$status" -ne 0 ]; then
  msg=$(sed 's/\x1b\[[0-9;]*m//g' "$out" | tr -d '\r' | grep -E -A3 '^error|error:|panicked at|^---- .* stdout ----|FAILED|What went wrong|Could not resolve|Caused by' | head -400 | cut -c1-300 | sed 's#/home/runner/work/[^/]*/[^/]*/##' | awk 'BEGIN{ORS="%0A"}{print}')
  printf '::error::%s\n' "$msg"
fi
exit "$status"

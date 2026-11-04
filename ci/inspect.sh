#!/usr/bin/env bash
# Diagnostics only: prints member signatures of Minecraft classes as notices, so version specific
# code can be written without a local game jar. Usage: ci/inspect.sh pkg.Class ...
set +e
jars=$(find "$HOME/.gradle" "$HOME/.cache" -name '*.jar' 2>/dev/null | grep -iE 'minecraft' | grep -viE 'sources|loader|fabric-api|intermediary-|yarn|libraries' | head -30)
echo "candidate jars:"; echo "$jars"
for cls in "$@"; do
  found=""
  for jar in $jars; do
    out=$(javap -p -cp "$jar" "$cls" 2>/dev/null)
    if [ -n "$out" ]; then found="$out"; break; fi
  done
  if [ -z "$found" ]; then found="(not found)"; fi
  msg=$(printf '%s\n' "$found" | cut -c1-220 | head -80 | awk 'BEGIN{ORS="%0A"}{print}')
  printf '::notice title=%s::%s\n' "$cls" "$msg"
done

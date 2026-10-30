#!/usr/bin/env bash
# Builds FrostClient-x86_64.AppImage from target/release/frost-launcher.
# Usage: packaging/linux/build-appimage.sh [version]
set -euo pipefail

VERSION="${1:-0.1.0}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/dist"
APPDIR="$ROOT/target/AppDir"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps" "$OUT"

install -m 755 "$ROOT/target/release/frost-launcher" "$APPDIR/usr/bin/frost-launcher"
install -m 644 "$ROOT/packaging/linux/frost-client.desktop" "$APPDIR/frost-client.desktop"
install -m 644 "$ROOT/packaging/linux/frost-client.desktop" "$APPDIR/usr/share/applications/frost-client.desktop"
install -m 644 "$ROOT/packaging/linux/frost-client.png" "$APPDIR/frost-client.png"
install -m 644 "$ROOT/packaging/linux/frost-client.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/frost-client.png"

cat > "$APPDIR/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/usr/bin/frost-launcher" "$@"
EOF
chmod +x "$APPDIR/AppRun"

TOOL="$ROOT/target/appimagetool"
if [ ! -x "$TOOL" ]; then
  curl -fsSL -o "$TOOL" \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage"
  chmod +x "$TOOL"
fi

# --appimage-extract-and-run: works on CI runners without FUSE
ARCH=x86_64 "$TOOL" --appimage-extract-and-run "$APPDIR" "$OUT/FrostClient-$VERSION-x86_64.AppImage"
echo "built $OUT/FrostClient-$VERSION-x86_64.AppImage"

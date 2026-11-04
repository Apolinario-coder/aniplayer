#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${APPIMAGETOOL:?Set APPIMAGETOOL to the path of appimagetool}"
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app="$stage/AniPlayer.AppDir"
install -Dm755 target/release/ani-player "$app/usr/bin/ani-player"
install -m644 assets/ani-player.png "$app/ani-player.png"
install -Dm644 assets/i18n/Noto-OFL.txt "$app/usr/share/doc/ani-player/Noto-OFL.txt"
install -Dm644 assets/anime4k/LICENSE "$app/usr/share/doc/ani-player/Anime4K-LICENSE"
cat > "$app/ani-player.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=Ani Player
Exec=ani-player
Icon=ani-player
Categories=AudioVideo;Player;
Terminal=false
EOF
cat > "$app/AppRun" <<'EOF'
#!/bin/sh
HERE=$(dirname "$(readlink -f "$0")")
exec "$HERE/usr/bin/ani-player" "$@"
EOF
chmod +x "$app/AppRun"
mkdir -p dist
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$APPIMAGETOOL" "$app" "dist/Ani-Player-$version-linux-x86_64.AppImage"

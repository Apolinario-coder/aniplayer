#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
: "${APPIMAGETOOL:?Set APPIMAGETOOL to the path of appimagetool}"
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app="$stage/AniPlayer.AppDir"
install -Dm755 target/release/ani-player "$app/usr/bin/ani-player"
if [[ -n ${ANI_LINUX_MEDIA_DIR:-} ]]; then
  source installer/bundle-media.sh
  bundle_media "$ANI_LINUX_MEDIA_DIR" "$app/usr/lib/ani-player/media" librife.so
fi
for soname in libxkbcommon-x11.so.0 libxkbcommon.so.0 libxcb-xkb.so.1; do
  library=$(ldconfig -p | awk -v name="$soname" '$1 == name && !seen++ {print $NF}')
  test -n "$library"
  install -Dm644 "$library" "$app/usr/lib/$soname"
done
for package in libxkbcommon-x11-0 libxkbcommon0 libxcb-xkb1; do
  install -Dm644 "/usr/share/doc/$package/copyright" "$app/usr/share/doc/ani-player/$package-copyright"
done
install -m644 assets/ani-player.png "$app/ani-player.png"
install -Dm644 assets/i18n/Noto-OFL.txt "$app/usr/share/doc/ani-player/Noto-OFL.txt"
install -Dm644 assets/anime4k/LICENSE "$app/usr/share/doc/ani-player/Anime4K-LICENSE"
install -Dm644 assets/icons/LICENSE "$app/usr/share/doc/ani-player/Lucide-LICENSE"
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
export LD_LIBRARY_PATH="$HERE/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
if [ -d "$HERE/usr/lib/ani-player/media" ]; then
  export ANI_MEDIA_DIR="$HERE/usr/lib/ani-player/media"
fi
exec "$HERE/usr/bin/ani-player" "$@"
EOF
chmod +x "$app/AppRun"
mkdir -p dist
APPIMAGE_EXTRACT_AND_RUN=1 ARCH=x86_64 "$APPIMAGETOOL" "$app" "dist/Ani-Player-$version-linux-x86_64.AppImage"

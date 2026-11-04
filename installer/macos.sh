#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c 'import json,sys; print(json.load(sys.stdin)["packages"][0]["version"])')
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
app="$stage/root/Ani Player.app/Contents"
mkdir -p "$app/MacOS" "$app/Resources" "$stage/AniPlayer.iconset" dist
lipo -create target/aarch64-apple-darwin/release/ani-player target/x86_64-apple-darwin/release/ani-player -output "$app/MacOS/ani-player"
chmod +x "$app/MacOS/ani-player"
cat > "$app/MacOS/launch" <<'EOF'
#!/bin/sh
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
exec "$(dirname "$0")/ani-player" "$@"
EOF
chmod +x "$app/MacOS/launch"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" assets/ani-player.png --out "$stage/AniPlayer.iconset/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" assets/ani-player.png --out "$stage/AniPlayer.iconset/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$stage/AniPlayer.iconset" -o "$app/Resources/AniPlayer.icns"
cp assets/i18n/Noto-OFL.txt "$app/Resources/"
cp assets/anime4k/LICENSE "$app/Resources/Anime4K-LICENSE"
cp assets/icons/LICENSE "$app/Resources/Lucide-LICENSE"
cat > "$app/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>launch</string>
<key>CFBundleIconFile</key><string>AniPlayer.icns</string>
<key>CFBundleIdentifier</key><string>com.apolinario.ani-player</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleName</key><string>Ani Player</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>LSMinimumSystemVersion</key><string>12.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
plutil -lint "$app/Info.plist"
codesign --force --sign - "$app/MacOS/ani-player"
codesign --force --sign - "$stage/root/Ani Player.app"
pkgbuild --root "$stage/root" --install-location /Applications --identifier com.apolinario.ani-player --version "$version" "dist/Ani-Player-$version-macOS-universal.pkg"

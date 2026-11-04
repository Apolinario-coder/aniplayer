#!/usr/bin/env bash
set -euo pipefail
destination=$(realpath -m "$1")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$destination"/{bin,lib,rife/models/rife-v4.16lite,licenses}
cd "$work"
git clone --depth 1 --branch R62 https://github.com/vapoursynth/vapoursynth.git
cd vapoursynth
./autogen.sh
./configure --prefix="$destination"
make -j2
make install
cp COPYING.LESSER "$destination/licenses/VapourSynth-LGPL.txt"
cd "$work"
git clone --depth 1 --branch v0.36.0 https://github.com/mpv-player/mpv.git
cd mpv
PKG_CONFIG_PATH="$destination/lib/pkgconfig" meson setup build --prefix="$destination" -Dvapoursynth=enabled -Dlibmpv=false -Dmanpage-build=disabled
meson compile -C build -j2
meson install -C build
cp Copyright LICENSE.GPL "$destination/licenses/"
cp /usr/bin/ffmpeg "$destination/bin/ffmpeg.real"
cp /usr/bin/python3.10 "$destination/bin/python3.real"
cp -a /usr/lib/python3.10 "$destination/lib/"
module=$(find "$destination/lib" -name 'vapoursynth*.so' -print -quit)
test -n "$module"
mkdir -p "$destination/lib/python3.10/site-packages"
cp "$module" "$destination/lib/python3.10/site-packages/"
cp "$destination/bin/mpv" "$destination/bin/mpv.real"
base=https://github.com/styler00dollar/VapourSynth-RIFE-ncnn-Vulkan
curl -fLsS --retry 3 "$base/releases/download/r9_mod_v33/librife_linux_x86-64.so" -o "$destination/rife/librife.so"
raw=https://raw.githubusercontent.com/styler00dollar/VapourSynth-RIFE-ncnn-Vulkan/c3ec6aabc07c8fa37a4f58d7fed9e2ad1fc1b13f
for file in flownet.bin flownet.param; do
  curl -fLsS --retry 3 "$raw/models/rife-v4.16_lite_ensembleFalse/$file" -o "$destination/rife/models/rife-v4.16lite/$file"
done
curl -fLsS --retry 3 "$raw/LICENSE" -o "$destination/licenses/RIFE.txt"
python3 -m pip download --no-deps --dest "$work" yt-dlp==2026.10.21
python3 -m zipfile -e "$work"/yt_dlp-*.whl "$destination/lib/python3.10/site-packages"
cat > "$destination/bin/yt-dlp" <<'EOF'
#!/bin/sh
exec "$(dirname "$0")/python3" -m yt_dlp "$@"
EOF
for executable in mpv ffmpeg python3; do
  cat > "$destination/bin/$executable" <<'EOF'
#!/bin/sh
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd -P)
export LD_LIBRARY_PATH="$ROOT/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PYTHONHOME="$ROOT"
export PYTHONPATH="$ROOT/lib/python3.10/site-packages"
export PATH="$ROOT/bin:$PATH"
exec "$ROOT/bin/$(basename "$0").real" "$@"
EOF
done
chmod +x "$destination/bin/"*
while IFS= read -r -d '' file; do
  LD_LIBRARY_PATH="$destination/lib" ldd "$file" 2>/dev/null |
    awk '/=> \/.* \(/ {print $3}' || true
done < <(find "$destination" -type f \( -name '*.so*' -o -name '*.real' \) -print0) |
  sort -u > "$work/libraries"
while IFS= read -r library; do
  case "$(basename "$library")" in
    libc.so.*|libm.so.*|libpthread.so.*|libdl.so.*|librt.so.*|ld-linux*) continue ;;
  esac
  target="$destination/lib/$(basename "$library")"
  if [[ "$library" != "$target" ]]; then cp -L "$library" "$target"; fi
done < "$work/libraries"
dpkg-query -W -f='${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\n' > "$destination/licenses/ubuntu-packages.tsv"
for package in ffmpeg python3.10 libzimg2 libavcodec58 libavformat58 libavfilter7 libplacebo192 libstdc++6; do
  cp "/usr/share/doc/$package/copyright" "$destination/licenses/$package-copyright"
done
echo '18a00a5e3ac90a5dfcfdf85fdb81d977381a5a2560a4c45dbe9a469d007b1886  rife/librife.so' |
  (cd "$destination" && sha256sum -c -)
"$destination/bin/mpv" --vf=help | grep vapoursynth
"$destination/bin/python3" -c 'import vapoursynth; print(vapoursynth.core.version())'

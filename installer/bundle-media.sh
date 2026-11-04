#!/usr/bin/env bash

bundle_media() {
  local source=$1 destination=$2 plugin=$3
  local tool
  for tool in mpv ffmpeg python3 yt-dlp; do
    test -x "$source/bin/$tool" || { printf 'Missing media tool: %s\n' "$source/bin/$tool" >&2; return 1; }
  done
  test -d "$source/licenses"
  test -s "$source/rife/$plugin"
  test -s "$source/rife/models/rife-v4.16lite/flownet.bin"
  test -s "$source/rife/models/rife-v4.16lite/flownet.param"
  mkdir -p "$destination"
  cp -a "$source/." "$destination/"
}

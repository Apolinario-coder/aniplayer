# Ani Player

Ani Player is a free desktop video player for anime. Its Rust interface organizes local episodes into folders and seasons. mpv handles playback.

![Ani Player's full-screen menu with episode controls and artwork of a train arriving at a station](docs/full-screen.png)

## Features

- **Episode library.** Add local files or M3U playlists, organize episodes in nested folders, and pick up your saved queue after restarting. Hover over a local episode to preview short clips.
- **Full-screen mode.** Press **F11** for the full Ani Player experience. The full-screen hub puts episodes, audio, subtitles, and video settings within easy reach. On Windows, it also supports controllers and offers a settings panel while the video plays.
- **Anime4K.** Choose a preset in **Motion → Anime4K**. The shaders run in mpv, with output sized to the video area.
- **RIFE interpolation.** Select a frame-rate target in **Motion**. The release packages include VapourSynth, the RIFE plug-in, and its model. Interpolation runs on a compatible GPU; performance depends on the video and your hardware.
- **Playback controls.** Switch audio tracks, load external subtitles, and jump to chapters. FFmpeg handles MKV export and thumbnail generation. HLS sharing also uses Python.
- **Seven interface languages.** Choose one under **Tools → Configuration → Language** or from the full-screen menu.

## Download and run

Use the package for your platform from [Releases](https://github.com/Apolinario-coder/aniplayer/releases), then choose **Open episode** or drop a file into the player. All three packages include mpv, FFmpeg, Python, VapourSynth, yt-dlp, and the RIFE plug-in and model. You do not need to install these tools separately.

| Platform | Package | Playback |
| --- | --- | --- |
| Windows x64 | Run `Ani-Player-1.0.0-windows-x64-setup.exe` and follow the installer. | Video plays inside Ani Player. |
| Linux x86-64 | Run `chmod +x Ani-Player-*.AppImage` and open the AppImage. Use a distribution with glibc 2.35 or newer. | Video opens in an mpv window. |
| macOS 15 or later, Apple Silicon or Intel | Open `Ani-Player-1.0.0-macOS-universal.pkg` and launch Ani Player from Applications. The PKG is unsigned. | Video opens in an mpv window. |

Linux and macOS use separate playback windows. Embedded playback, the in-video settings panel, and XInput controller navigation are Windows features.

RIFE needs a compatible graphics driver. Windows and Linux use Vulkan; macOS includes MoltenVK to run it through Metal. Playback, export, and the mpv/VapourSynth connection are checked in CI. RIFE frame generation is tested on Windows hardware and Linux software Vulkan; GPU performance on Linux and macOS has not been measured.

If your Linux system cannot mount AppImages, run `./Ani-Player-1.0.0-linux-x86_64.AppImage --appimage-extract-and-run`.

Ani Player checks GitHub at startup for a newer stable release. **Download update** opens the release in your browser, where you can choose your platform's package.

## Build from source

Install stable Rust and the system development libraries for your platform. Linux needs OpenGL, X11 or Wayland, and xkbcommon development libraries. Windows builds use `rc.exe` with MSVC or `windres` with GNU. macOS builds use Xcode command-line tools.

```sh
cargo build --release --locked
cargo test --release --locked
```

The executable is in `target/release`. A plain Cargo build uses media tools available on your system. You can override their locations with `ANI_MPV`, `ANI_FFMPEG`, and `ANI_PYTHON`.

The [package workflow](.github/workflows/packages.yml) builds the complete release installers and AppImage. In PowerShell, run `./installer/Build-Installer.ps1 -ReleaseRuntime` to build the Windows installer. The build downloads the components listed in [windows-runtime.txt](installer/windows-runtime.txt) and checks their SHA-256 hashes. It also needs 7-Zip and Visual Studio's C++ redistributable files. Use `-RuntimeRoot <path>` for runtime files you supply, or `-SystemRuntime` for an installer that uses tools already on the user's system. Linux and macOS runtime preparation is in [prepare-linux-media.sh](installer/prepare-linux-media.sh) and [prepare-macos-media.py](installer/prepare-macos-media.py).

When [reporting a bug](https://github.com/Apolinario-coder/aniplayer/issues), include the Ani Player and mpv versions, your operating system, and a short way to reproduce it.

## Credits

- [mpv](https://mpv.io/) handles video and audio playback.
- [FFmpeg](https://ffmpeg.org/) supplies export, previews, and HLS encoding.
- [Anime4K](https://github.com/bloc97/Anime4K) supplies the [vendored shaders](assets/anime4k/LICENSE).
- [VapourSynth](https://www.vapoursynth.com/) hosts the optional RIFE video filter.
- [egui and eframe](https://github.com/emilk/egui) supply the interface and native window.
- [Noto Sans CJK](https://github.com/notofonts/noto-cjk) supplies the bundled Chinese and Japanese font subsets. See the [font license](assets/i18n/Noto-OFL.txt).

## Source Code of Bundled Third-Party Components

See [Cargo.lock](Cargo.lock) for the Rust dependency list. Media license notices are included in each package: `licenses` on Windows, `usr/lib/ani-player/media/licenses` inside the AppImage, and `Contents/Resources/media/<architecture>/licenses` inside the macOS app. Linux includes an Ubuntu package/source version manifest; macOS includes Homebrew formula metadata with source URLs and build revisions.

| Component | Version or revision | License | Source code |
| --- | --- | --- | --- |
| Anime4K shaders | `7684e95` | [MIT](assets/anime4k/LICENSE) | [bloc97/Anime4K](https://github.com/bloc97/Anime4K/tree/7684e9586f8dcc738af08a1cdceb024cc184f426) |
| Lucide icons | `91a1359` | [ISC and MIT](assets/icons/LICENSE) | [lucide-icons/lucide](https://github.com/lucide-icons/lucide/tree/91a1359b5ea235f620ba3a9682015ab8d6ada3af) |
| Noto Sans CJK SC and JP | 2.004, subsetted | [SIL OFL 1.1](assets/i18n/Noto-OFL.txt) | [notofonts/noto-cjk](https://github.com/notofonts/noto-cjk/tree/Sans2.004/Sans/OTF) |
| egui and eframe | 0.31.1 | MIT or Apache-2.0 | [emilk/egui](https://github.com/emilk/egui/tree/0.31.1) |
| AppImage Type 2 runtime | `fedfad5` | [MIT and dependency notices](https://github.com/AppImage/type2-runtime/blob/fedfad5/LICENSE) | [AppImage/type2-runtime](https://github.com/AppImage/type2-runtime/tree/fedfad5) |
| libxkbcommon and libxkbcommon-x11 (Linux) | Ubuntu `1.4.0-1` | MIT-style | [Ubuntu source package](https://launchpad.net/ubuntu/+source/libxkbcommon/1.4.0-1) |
| libxcb-xkb (Linux) | Ubuntu `1.14-3ubuntu3` | MIT/X11 | [Ubuntu source package](https://launchpad.net/ubuntu/+source/libxcb/1.14-3ubuntu3) |
| mpv (Windows) | `46787be250`, shinchiro build 20261104 | GPL; see bundled Copyright | [mpv source](https://github.com/mpv-player/mpv/tree/46787be250), [build scripts](https://github.com/shinchiro/mpv-winbuild-cmake) |
| FFmpeg (Windows) | `200a98d5b`, shinchiro build 20261104 | GPL-3.0-or-later build | [FFmpeg source](https://github.com/FFmpeg/FFmpeg/tree/200a98d5b), [build scripts](https://github.com/shinchiro/mpv-winbuild-cmake) |
| Python (Windows) | 3.12.10 embeddable | PSF and bundled notices | [CPython](https://github.com/python/cpython/tree/v3.12.10) |
| VapourSynth (Windows) | R79 | LGPL-2.1-or-later | [vapoursynth/vapoursynth](https://github.com/vapoursynth/vapoursynth/tree/R79) |
| mpv (Linux / macOS) | 0.36.0 / 0.41.0 with Homebrew revisions | GPL; see bundled notices | [Linux source](https://github.com/mpv-player/mpv/tree/v0.36.0), [macOS source](https://github.com/mpv-player/mpv/tree/v0.41.0) |
| FFmpeg (Linux / macOS) | Ubuntu 4.4.2 / Homebrew 9.0.2 (ARM), 9.0.1 (Intel) | GPL builds; see bundled notices | [Ubuntu source package](https://launchpad.net/ubuntu/+source/ffmpeg), [ARM source](https://github.com/FFmpeg/FFmpeg/tree/n9.0.2), [Intel source](https://github.com/FFmpeg/FFmpeg/tree/n9.0.1) |
| Python (Linux / macOS) | Ubuntu 3.10 / 3.15.2 (ARM), 3.14.7 (Intel) | PSF and bundled notices | [CPython](https://github.com/python/cpython), [Ubuntu source package](https://launchpad.net/ubuntu/+source/python3.10) |
| VapourSynth (Linux / macOS) | R62 / R81 (ARM), R79 (Intel) | LGPL-2.1-or-later | [R62](https://github.com/vapoursynth/vapoursynth/tree/R62), [R81](https://github.com/vapoursynth/vapoursynth/tree/R81), [R79](https://github.com/vapoursynth/vapoursynth/tree/R79) |
| MoltenVK (macOS) | 1.4.2 | Apache-2.0 | [KhronosGroup/MoltenVK](https://github.com/KhronosGroup/MoltenVK/tree/v1.4.2) |
| Vulkan loader (macOS) | 1.4.363.0 (ARM), 1.4.357.0 (Intel) | Apache-2.0 | [KhronosGroup/Vulkan-Loader](https://github.com/KhronosGroup/Vulkan-Loader) |
| Homebrew media dependencies (macOS) | Exact versions in packaged `licenses/homebrew.json` | Per formula; see bundled notices | [Homebrew build recipes](https://github.com/Homebrew/homebrew-core) |
| RIFE plug-in and 4.16 Lite model (all platforms) | `r9_mod_v33` | MIT | [VapourSynth-RIFE-ncnn-Vulkan](https://github.com/styler00dollar/VapourSynth-RIFE-ncnn-Vulkan/tree/c3ec6aabc07c8fa37a4f58d7fed9e2ad1fc1b13f) |
| yt-dlp (all platforms) | 2026.10.21 | Unlicense; Windows combined executable includes GPLv3+ code | [yt-dlp](https://github.com/yt-dlp/yt-dlp/tree/2026.10.21) |

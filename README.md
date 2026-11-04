# Ani Player

Ani Player is a free desktop video player for anime. Its Rust interface organizes local episodes into folders and seasons. mpv handles playback.

![Ani Player's full-screen menu with episode controls and artwork of a train arriving at a station](docs/full-screen.png)

## Features

- **Episode library.** Add local files or M3U playlists, organize episodes in nested folders, and pick up your saved queue after restarting. Hover over a local episode to preview short clips.
- **Full-screen mode.** Press **F11** for the full Ani Player experience. The full-screen hub puts episodes, audio, subtitles, and video settings within easy reach. On Windows, it also supports controllers and offers a settings panel while the video plays.
- **Anime4K.** Choose a preset in **Motion → Anime4K**. The shaders run in mpv, with output sized to the video area.
- **RIFE interpolation.** Select a frame-rate target in **Motion**. This requires an mpv build with VapourSynth, the RIFE plug-in and model, and a compatible GPU runtime.
- **Playback controls.** Switch audio tracks, load external subtitles, and jump to chapters. FFmpeg handles MKV export and thumbnail generation. HLS sharing also uses Python.
- **Seven interface languages.** Choose one under **Tools → Configuration → Language** or from the full-screen menu.

## Download and run

Use the package for your platform from [Releases](https://github.com/Apolinario-coder/aniplayer/releases), then choose **Open episode** or drop a file into the player. The Windows installer includes mpv, FFmpeg, Python, VapourSynth, yt-dlp, and the RIFE plug-in and model. The current Linux and macOS downloads require system-installed media tools.

| Platform | Package | Playback |
| --- | --- | --- |
| Windows x64 | Run `Ani-Player-1.0.0-windows-x64-setup.exe` and follow the installer. | Video plays inside Ani Player. |
| Linux x86-64 | Run `chmod +x Ani-Player-*.AppImage` and open the AppImage. Use a distribution with glibc 2.35 or newer. | Video opens in an mpv window. |
| macOS 12 or later, Apple Silicon or Intel | Open the PKG and launch Ani Player from Applications. The PKG is unsigned. | Video opens in an mpv window. |

Linux and macOS use separate playback windows. Embedded playback, the in-video settings panel, and XInput controller navigation are Windows features. GPU RIFE on Linux and macOS requires a compatible local setup and has not been verified on those platforms.

On Linux and macOS, install mpv and FFmpeg for playback, yt-dlp for online URLs, and Python 3 for HLS. The macOS app searches the standard Homebrew paths. You can point Ani Player directly to its tools with `ANI_MPV`, `ANI_FFMPEG`, and `ANI_PYTHON`.

For Ubuntu or Debian, install the local playback tools with `sudo apt install mpv ffmpeg python3 curl`. On macOS with Homebrew, use `brew install mpv ffmpeg yt-dlp`.

Ani Player checks GitHub at startup for a newer stable release. **Download update** opens the release in your browser, where you can choose your platform's package.

## Build from source

Install stable Rust and the system development libraries for your platform. Linux needs OpenGL, X11 or Wayland, and xkbcommon development libraries. Windows builds use `rc.exe` with MSVC or `windres` with GNU. macOS builds use Xcode command-line tools.

```sh
cargo build --release --locked
cargo test --release --locked
```

The executable is in `target/release`. The [package workflow](.github/workflows/packages.yml) builds the release installers and AppImage. In PowerShell, run `./installer/Build-Installer.ps1 -ReleaseRuntime` to build the complete Windows installer. The build downloads the components listed in [windows-runtime.txt](installer/windows-runtime.txt) and checks their SHA-256 hashes. Use `-RuntimeRoot <path>` for runtime files you supply, or `-SystemRuntime` for an installer that uses tools already on the user's system.

When [reporting a bug](https://github.com/Apolinario-coder/aniplayer/issues), include the Ani Player and mpv versions, your operating system, and a short way to reproduce it.

## Credits

- [mpv](https://mpv.io/) handles video and audio playback.
- [FFmpeg](https://ffmpeg.org/) supplies export, previews, and HLS encoding.
- [Anime4K](https://github.com/bloc97/Anime4K) supplies the [vendored shaders](assets/anime4k/LICENSE).
- [VapourSynth](https://www.vapoursynth.com/) hosts the optional RIFE video filter.
- [egui and eframe](https://github.com/emilk/egui) supply the interface and native window.
- [Noto Sans CJK](https://github.com/notofonts/noto-cjk) supplies the bundled Chinese and Japanese font subsets. See the [font license](assets/i18n/Noto-OFL.txt).

## Source Code of Bundled Third-Party Components

See [Cargo.lock](Cargo.lock) for the full Rust dependency list. Windows media components come from the public builds listed below. Their license notices are installed in the player's `licenses` folder.

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
| RIFE plug-in and 4.16 Lite model (Windows) | `r9_mod_v33` | MIT | [VapourSynth-RIFE-ncnn-Vulkan](https://github.com/styler00dollar/VapourSynth-RIFE-ncnn-Vulkan/tree/c3ec6aabc07c8fa37a4f58d7fed9e2ad1fc1b13f) |
| yt-dlp executable (Windows) | 2026.10.21 | GPLv3+ combined executable; Unlicense for yt-dlp code | [yt-dlp](https://github.com/yt-dlp/yt-dlp/tree/2026.10.21) |

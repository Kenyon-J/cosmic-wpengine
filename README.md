<div align="center">
  
# cosmic-wallpaper

[![Rust](https://img.shields.io/badge/rust-stable-blue.svg?logo=rust)](https://www.rust-lang.org)
[![Wayland](https://img.shields.io/badge/wayland-native-success?logo=linux)](https://wayland.freedesktop.org/)
[![COSMIC](https://img.shields.io/badge/optimized_for-COSMIC-orange)](#)

**A Wayland-native live wallpaper engine optimized for the [COSMIC desktop](https://system76.com/cosmic), written in Rust.**

<br />

<img src="https://jkenyon.co.uk/images/cosmic-wpengine.png" alt="cosmic-wallpaper showcase" width="100%" />

</div>

---

## ✨ Features


* 🎬 **Video Backgrounds**: Play generic video files (.mp4, .webm) natively as your desktop background, leveraging `ffmpeg-next` and `wgpu`.
* 🎵 **Media Integration**: Displays album art from MPRIS-compatible players (Spotify, VLC, Firefox, etc.).
* 🖼️ **Artwork Fallback**: Queries the iTunes API for cover art if local artwork is unavailable (e.g., due to sandboxing).
* 🎞️ **Spotify Canvas**: Fetches and plays looping video backgrounds for supported tracks via FFmpeg *(Note: Requires a local Canvas API proxy)*.
* 🎤 **Synced Lyrics**: Uses the LRCLIB API to display time-synced lyrics with audio-reactive, physics-based animations.
* 🎧 **Audio Visualizer**: Captures system audio via PipeWire and renders an FFT-based visualizer with high-performance pre-calculated DSP windowing and customizable styles.
* 🌌 **Desktop Integration**: Reads the active COSMIC desktop wallpaper — images, solid colours, and gradients — to render native transparent and frosted-glass effects, and plays nicely with COSMIC 1.3's system-wide frosted glass.
* ⚙️ **Settings GUI & Tray**: Includes a `libcosmic`-based configuration app and a D-Bus system tray applet for managing settings.
* 🔄 **Self-Updater**: One-click updates from the settings app, verified end-to-end — SHA-256 checksums signed with minisign, checked against a key embedded in the binary.
* 🌦️ **Weather Effects**: Uses GPU compute shaders to render weather particles (rain, snow) based on local Open-Meteo data.
* 🐧 **Wayland Support**: Built with `smithay-client-toolkit` (`wlr-layer-shell`) and `wgpu`, fully supporting multi-monitor setups and fractional scaling.

## 🏗️ Architecture

The engine is highly parallelized. Subsystems run concurrently using a hybrid approach: most components use `tokio` asynchronous tasks, while reactive event-based monitors (like the MPRIS watcher) utilize dedicated OS threads to ensure real-time responsiveness without overloading the async scheduler. Events are channeled to the main `wgpu` render loop:

```
cosmic-wallpaper/
├── src/
│   ├── main.rs              # Async runtime entry point
│   └── modules/
│       ├── config/          # Hot-reloading TOML config + COSMIC wallpaper resolution
│       ├── state/           # Render-state interpolation & easing
│       ├── event/           # Concurrency event messaging
│       ├── mpris/           # D-Bus media player integration & album art
│       ├── lrclib/          # Time-synced lyrics API integration
│       ├── video/           # FFmpeg background video decoding
│       ├── gui/             # libcosmic settings app + signed self-updater
│       ├── tray.rs          # System tray menu and GUI launcher
│       ├── audio.rs         # PipeWire capture & FFT computation
│       ├── weather/         # Open-Meteo API polling
│       ├── renderer/        # wgpu multi-surface compositor and text rendering
│       ├── wayland.rs       # wlr-layer-shell surface management
│       ├── colour/          # K-Means palette extraction & contrast checks
│       ├── visualiser_pass.rs   # User-theme shader pipeline loading
│       ├── album_art.wgsl   # Frosted glass, dropshadows & art
│       ├── ambient.wgsl     # Procedural sky & weather patterns
│       ├── visualiser.wgsl  # Audio-reactive frequency blooms
│       ├── text.wgsl        # Glyph atlas text rendering
│       ├── weather_render.wgsl  # Weather particle rendering
│       └── weather_compute.wgsl # Weather particle compute physics
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime — coordinates subsystems concurrently |
| `smithay-client-toolkit` | Wayland protocol client, including wlr-layer-shell |
| `wgpu` | Cross-platform GPU rendering |
| `mpris` | MPRIS D-Bus media player integration |
| `pipewire` | PipeWire audio capture |
| `rustfft` | Fast Fourier Transform for audio visualisation |
| `ksni` | D-Bus system tray integration |
| `image` | Album art decoding |
| `palette` | Colour manipulation |
| `reqwest` | HTTP client for weather, LRCLIB, iTunes fallback, and release updates |
| `ffmpeg-next` | Video background and Spotify Canvas decoding |
| `libcosmic` | Settings GUI toolkit (pinned to a tested rev) |
| `minisign-verify` | Verifies release signatures before self-update |
| `serde` + `toml` | Config file serialisation |

## Installation

Grab the latest release from the [Releases page](https://github.com/Kenyon-J/cosmic-wpengine/releases):

- **Pop!_OS / Ubuntu 24.04**: download the `.deb` and install it — it ships both
  binaries plus an autostart entry, so the engine starts with your session:
  ```bash
  sudo apt install ./cosmic-wallpaper_*.deb
  ```
- **Other distros**: download the prebuilt `cosmic-wallpaper-x86_64-linux-gnu`
  and `cosmic-wallpaper-gui-x86_64-linux-gnu` binaries. `SHA256SUMS.txt` is
  signed with minisign (`SHA256SUMS.txt.minisig`) so you can verify what you run.
- Once installed, future updates are one click in the settings app: it checks
  GitHub Releases, verifies the signed checksums, and swaps the binaries in place.

Or build from source — see [Building](#building).

## Prerequisites

- COSMIC desktop with `cosmic-comp` compositor
- PipeWire (standard on modern Linux)
- A media player that supports MPRIS (Spotify, VLC, Firefox, etc.)

### System Dependencies

To compile the application, you'll need the PipeWire development headers and Clang (required by `bindgen` to generate the PipeWire Rust bindings).

To compile you also need the Wayland, EGL, and FFmpeg development headers
(the same set CI builds with):

**Ubuntu 24.04 / Pop!_OS 24.04:**
```bash
sudo apt-get update && sudo apt-get install -y clang libclang-dev libpipewire-0.3-dev \
    pkg-config libxkbcommon-dev libwayland-dev wayland-protocols libegl1-mesa-dev \
    libdbus-1-dev ffmpeg libssl-dev libavutil-dev libavformat-dev libavfilter-dev \
    libavdevice-dev libavcodec-dev libswscale-dev libswresample-dev libpostproc-dev
```
**Arch Linux:**
```bash
sudo pacman -S base-devel clang pipewire pkgconf rust libxkbcommon wayland \
    wayland-protocols dbus ffmpeg
```

## Building

```bash
# Install Rust if you haven't
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/Kenyon-J/cosmic-wpengine
cd cosmic-wpengine
# This builds both the main engine and the cosmic-wallpaper-gui binary
cargo build --release --locked --all-targets

# Install both binaries to ~/.local/bin (or anywhere on your PATH)
install -Dm755 target/release/cosmic-wallpaper ~/.local/bin/cosmic-wallpaper
install -Dm755 target/release/cosmic-wallpaper-gui ~/.local/bin/cosmic-wallpaper-gui

# Try it out in the foreground (Ctrl+C to stop)
cosmic-wallpaper

# Or launch it detached, so it survives closing the terminal
nohup cosmic-wallpaper >/dev/null 2>&1 & disown
```

## Autostart on Login

To start the engine automatically with your COSMIC session, install the
bundled autostart entry with the full path to wherever you installed the
binary (`cosmic-session` does not search `~/.local/bin` or `~/.cargo/bin`):

```bash
mkdir -p ~/.config/autostart
sed "s|^Exec=.*|Exec=$HOME/.local/bin/cosmic-wallpaper|" \
    io.github.kenyon_j.cosmic_wpengine.autostart.desktop \
    > ~/.config/autostart/io.github.kenyon_j.cosmic_wpengine.desktop
```

If you installed via a system package (binary in `/usr/bin`), the file works
as-is:

```bash
cp io.github.kenyon_j.cosmic_wpengine.autostart.desktop \
   ~/.config/autostart/io.github.kenyon_j.cosmic_wpengine.desktop
```

## Configuration

On first run, a default config is created at `~/.config/cosmic-wallpaper/config.toml`:

```toml
mode = "auto"   # auto | album_art | audio_visualiser | weather
fps = 30

[weather]
enabled = false
latitude = 51.5    # your latitude
longitude = -0.1   # your longitude
poll_interval_minutes = 15
temperature_unit = "Celsius"   # Celsius | Fahrenheit

[appearance]
disable_blur = false            # disables the frosted-glass blur pass
blur_opacity = 0.4              # 0.0 sharp .. 1.0 fully blurred
transparent_background = false  # show the desktop wallpaper as-is
show_album_art = true
album_art_background = false    # blurred album art as the backdrop
album_color_background = true   # album palette colour as the backdrop
# custom_background_path = "/path/to/img.jpg"  # omit to sync with the COSMIC
#   wallpaper (images, solid colours and gradients all work)

[audio]
style = "monstercat"  # monstercat | bars | symmetric | waveform | your own theme
bands = 64            # number of frequency bands in visualiser
smoothing = 0.7       # 0.0 = instant, 1.0 = very smooth
show_lyrics = true
# canvas_proxy_url = "http://localhost:3000/api/canvas"  # opt-in Spotify Canvas proxy
```


## Video Backgrounds
You can put video files (like .mp4 or .webm) in your `~/.config/cosmic-wallpaper/videos` directory. The application will detect them and allow you to select them as your background using the tray settings menu!

## Custom Visualiser Themes

The visualizer fully supports custom user-made themes and WGSL shaders! On first run, default theme templates are automatically generated in `~/.config/cosmic-wallpaper/shaders/`.

To create a new theme:
1. Create a `.toml` file in the shaders directory (e.g., `~/.config/cosmic-wallpaper/shaders/my_theme.toml`):
   ```toml
   # Position elements using normalized screen coordinates (0.0 to 1.0)
   [album_art]
   position = [0.15, 0.8]
   size = 0.15
   shape = "square" # "circular" or "square"

   [track_info]
   position = [0.28, 0.75]
   align = "left" # "left", "center", or "right"

   [lyrics]
   position = [0.28, 0.85]
   align = "left"

   [weather]
   position = [0.98, 0.03]
   align = "right"

   [visualiser]
   shape = "linear" # "circular" or "linear"
   position = [0.5, 0.5]
   size = 1.0
   rotation = 0.0
   amplitude = 1.5
   # color_top = [1.0, 0.2, 0.5]      # Optional fixed colours (RGB 0.0 - 1.0)
   # color_bottom = [0.2, 0.5, 1.0]
   ```
2. Select your custom theme from the System Tray applet, or manually set `style = "my_theme"` in your main `config.toml`.
3. **Live Reloading:** Any edits you make to the `.toml` file while the wallpaper is running will be instantly applied to your desktop!
4. *(Advanced)* You can also provide a custom `my_theme.wgsl` shader file alongside your `.toml` to completely rewrite the graphics pipeline! For example:
   ```toml
   [album_art]
   position = [0.5, 0.5]
   size = 0.25
   shape = "circular"

   [visualiser]
   shape = "circular"
   position = [0.5, 0.5]
   size = 0.25
   amplitude = 1.0
   # Point this theme to your custom shader!
   shader = "radial_spin.wgsl"
   ```

### Writing Custom WGSL Shaders

If you provide a custom `shader = "my_shader.wgsl"` in your theme's `.toml`, place the `.wgsl` file in `~/.config/cosmic-wallpaper/shaders/`.
The engine will inject the following uniform struct. Ensure your custom shader uses this exact layout:

```wgsl
struct VisualiserUniforms {
    resolution: vec2<f32>,
    band_count: u32,
    lyric_pulse: f32,          // Beats snap to 1.0 and exponentially decay
    color_top: vec4<f32>,
    color_bottom: vec4<f32>,
    pos_size_rot: vec4<f32>,   // x: pos.x, y: pos.y, z: size, w: rotation (rads)
    amplitude: f32,
    shape: u32,                // 0 = circular, 1 = linear
    time: f32,                 // Elapsed time in seconds for scrolling effects!
    align: u32,                // 0 = left, 1 = center, 2 = right
    is_waveform: u32,          // bool
    _pad1: u32,
    _pad2: u32,
    _pad3: u32,
}

@group(0) @binding(0) var<uniform> uniforms: VisualiserUniforms;
@group(0) @binding(1) var<storage, read> bands: array<f32>;
```

## How it works

Each subsystem runs as an independent `tokio` task, sending events over async channels to the renderer:

```
MPRIS watcher  ──┐
PipeWire audio ──┼──[channel]──▶ Renderer ──▶ Wayland layer surface
Weather poller ──┘
```

The renderer processes events each frame, updates `AppState`, and dispatches to the appropriate WGSL shader via wgpu.

## Roadmap

- [x] System Tray (AppIndicator) menu for quick settings toggles (Blur, Lyrics, Backgrounds)
- [x] User-loadable custom shaders
- [x] Hardware-accelerated compute shaders for weather particles
- [x] Spotify Canvas (short video loops) background support
- [x] Advanced beat-detection for more organic lyric bouncing
- [ ] Interactive mouse-reactive wallpaper effects
- [x] Generic video file playback (MP4/WebM) as backgrounds
- [ ] Plugin API for custom data sources

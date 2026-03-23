<div align="center">
  
# 🌌 cosmic-wallpaper

[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue.svg?logo=rust)](https://www.rust-lang.org)
[![Wayland](https://img.shields.io/badge/wayland-native-success?logo=linux)](https://wayland.freedesktop.org/)
[![COSMIC](https://img.shields.io/badge/optimized_for-COSMIC-orange)](#)

**A next-generation, heavily reactive live wallpaper engine natively built for Wayland and the [COSMIC desktop](https://system76.com/cosmic), written in Rust.**

<br />

<!-- TODO: Replace with an actual hero image or WebM/GIF showcasing the wallpaper in action! -->
<img src="https://via.placeholder.com/800x400.png?text=Showcase+GIF+goes+here" alt="cosmic-wallpaper showcase" width="100%" />

</div>

---

## ✨ Features

* 🎵 **Dynamic Media Hub** — Displays perfectly scaled, frosted-glass-styled album art from any MPRIS-compatible player (Spotify, VLC, Tidal, Firefox, etc.).
* 🌐 **Smart Art Fallback** — Automatically queries the iTunes API for gorgeous high-res 600x600 covers if your local media player (e.g., Flatpak/Snap sandboxed apps) fails to provide artwork.
* 🎤 **Synced Karaoke Lyrics** — Seamlessly polls the free LRCLIB API to render perfectly synced, cleanly shadowed kinetic typography that physically springs and bounces to the beat!
* 📊 **Realtime Audio Visualiser** — Captures 32-bit float audio directly via PipeWire, feeding a zero-allocation Fast Fourier Transform (FFT) with **perceptual A-weighting** and **logarithmic frequency scaling**. Choose between responsive equalizer "Bars" or "Waveform" styles!
* 🖼️ **Native Desktop Integration** — Natively parses your active COSMIC desktop wallpaper to draw it natively into the wgpu render pass, bypassing Wayland layer isolation. Toggle "Transparent Background" on the fly for a stunning floating UI over your desktop!
* 🖱️ **System Tray Applet** — Control your wallpaper engine instantly with a built-in D-Bus system tray menu. Toggle lyrics, frosted blur, and background transparency with zero-latency hot-reloading.
* �️ **Weather & Time Reactive** — When media is paused, the background gracefully crossfades into a procedural WGSL weather engine. Experience rain streaks, snow, and drifting clouds synced to your local Open-Meteo conditions and time-of-day.
* 🚀 **Wayland Native** — Built on `smithay-client-toolkit` using `wlr-layer-shell` and `wgpu`. Fully supports HiDPI scaling, fractional rendering, and dynamic multi-monitor ultra-wide arrays out of the box.
* ⚙️ **Live Configuration** — Config files are hot-reloaded instantly via the system tray or `.toml` file edits.

## 🏗️ Architecture

The engine is highly parallelized. Subsystems run as independent `tokio` asynchronous tasks, channeling zero-copy events to the main `wgpu` render loop:

```
cosmic-wallpaper/
├── src/
│   ├── main.rs              # Async runtime entry point
│   └── modules/
│       ├── config.rs        # Hot-reloading TOML config
│       ├── state.rs         # Render-state interpolation & easing
│       ├── event.rs         # Concurrency event messaging
│       ├── mpris.rs         # D-Bus Media Player integration
│       ├── audio.rs         # PipeWire Capture & FFT computation
│       ├── weather.rs       # Open-Meteo API polling
│       ├── renderer.rs      # wgpu multi-surface compositor
│       ├── wayland.rs       # wlr-layer-shell surface management
│       └── colour.rs        # K-Means palette extraction
└── src/shaders/
    ├── album_art.wgsl       # Frosted glass, dropshadows & art
    ├── ambient.wgsl         # Procedural sky & weather patterns
    └── visualiser.wgsl      # Audio-reactive frequency blooms
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
| `reqwest` | HTTP client for weather API |
| `serde` + `toml` | Config file serialisation |

## Prerequisites

- COSMIC desktop with `cosmic-comp` compositor
- PipeWire (standard on modern Linux)
- A media player that supports MPRIS (Spotify, VLC, Firefox, etc.)

## Building

```bash
# Install Rust if you haven't
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/yourname/cosmic-wallpaper
cd cosmic-wallpaper
cargo build --release

# Run
./target/release/cosmic-wallpaper
```

## Configuration

On first run, a default config is created at `~/.config/cosmic-wallpaper/config.toml`:

```toml
mode = "auto"   # auto | album_art | audio_visualiser | weather
fps = 30

[weather]
enabled = false
latitude = 40.20   # your latitude
longitude = -67  # your longitude
poll_interval_minutes = 15

[appearance]
disable_blur = false
transparent_background = false
custom_background_path = "/path/to/img.jpg" # Omit to auto-sync with COSMIC!

[audio]
style = "bars"    # bars | waveform
bands = 64        # number of frequency bands in visualiser
smoothing = 0.7   # 0.0 = instant, 1.0 = very smooth
```

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
4. *(Advanced)* You can also provide a custom `my_theme.wgsl` shader file alongside your `.toml` to completely rewrite the graphics pipeline!

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
- [ ] Hardware-accelerated compute shaders for weather particles
- [ ] Spotify Canvas (short video loops) background support
- [x] Advanced beat-detection for more organic lyric bouncing

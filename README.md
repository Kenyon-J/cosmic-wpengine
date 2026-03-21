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
* ⚙️ **Live Configuration** — Config files are hot-reloaded instantly. *(Roadmap: Native libcosmic settings applet integration).*

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

## How it works

Each subsystem runs as an independent `tokio` task, sending events over async channels to the renderer:

```
MPRIS watcher  ──┐
PipeWire audio ──┼──[channel]──▶ Renderer ──▶ Wayland layer surface
Weather poller ──┘
```

The renderer processes events each frame, updates `AppState`, and dispatches to the appropriate WGSL shader via wgpu.

## Roadmap

- [ ] COSMIC settings panel integration via libcosmic
- [ ] System Tray (AppIndicator) menu for quick settings toggles (Blur, Lyrics, Backgrounds)
- [ ] User-loadable custom shaders
- [ ] Hardware-accelerated compute shaders for weather particles
- [ ] Spotify Canvas (short video loops) background support
- [ ] Advanced beat-detection for more organic lyric bouncing

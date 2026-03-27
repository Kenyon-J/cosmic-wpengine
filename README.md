<div align="center">
  
# cosmic-wallpaper

[![Rust](https://img.shields.io/badge/rust-1.75%2B-blue.svg?logo=rust)](https://www.rust-lang.org)
[![Wayland](https://img.shields.io/badge/wayland-native-success?logo=linux)](https://wayland.freedesktop.org/)
[![COSMIC](https://img.shields.io/badge/optimized_for-COSMIC-orange)](#)

**A Wayland-native live wallpaper engine optimized for the [COSMIC desktop](https://system76.com/cosmic), written in Rust.**

<br />

<!-- TODO: Replace with an actual hero image or WebM/GIF showcasing the wallpaper in action! -->
<img src="https://via.placeholder.com/800x400.png?text=Showcase+GIF+goes+here" alt="cosmic-wallpaper showcase" width="100%" />

</div>

---

## Features

* **Media Integration**: Displays album art from MPRIS-compatible players (Spotify, VLC, Firefox, etc.).
* **Artwork Fallback**: Queries the iTunes API for cover art if local artwork is unavailable (e.g., due to sandboxing).
* **Spotify Canvas**: Fetches and plays looping video backgrounds for supported tracks via FFmpeg *(Note: Requires a local Canvas API proxy)*.
* **Synced Lyrics**: Uses the LRCLIB API to display time-synced lyrics with audio-reactive, physics-based animations.
* **Audio Visualizer**: Captures system audio via PipeWire and renders an FFT-based visualizer with customizable styles.
* **Desktop Integration**: Reads the active COSMIC desktop wallpaper to render native transparent and frosted-glass effects.
* **Settings GUI & Tray**: Includes a `libcosmic`-based configuration app and a D-Bus system tray applet for managing settings.
* **Weather Effects**: Uses GPU compute shaders to render weather particles (rain, snow) based on local Open-Meteo data.
* **Wayland Support**: Built with `smithay-client-toolkit` (`wlr-layer-shell`) and `wgpu`, fully supporting multi-monitor setups and fractional scaling.

## Architecture

The engine is highly parallelized. Subsystems run as independent `tokio` asynchronous tasks, channeling events to the main `wgpu` render loop:

```
cosmic-wallpaper/
├── src/
│   ├── main.rs              # Async runtime entry point
│   └── modules/
│       ├── config.rs        # Hot-reloading TOML config
│       ├── state.rs         # Render-state interpolation & easing
│       ├── event.rs         # Concurrency event messaging
│       ├── mpris.rs         # D-Bus Media Player integration
│       ├── lrclib.rs        # Time-synced lyrics API integration
│       ├── video.rs         # FFmpeg background video decoding
│       ├── tray.rs          # System tray menu and GUI launcher
│       ├── audio.rs         # PipeWire Capture & FFT computation
│       ├── weather.rs       # Open-Meteo API polling
│       ├── renderer.rs      # wgpu multi-surface compositor
│       ├── wayland.rs       # wlr-layer-shell surface management
│       ├── colour.rs        # K-Means palette extraction
│       ├── album_art.wgsl   # Frosted glass, dropshadows & art
│       ├── ambient.wgsl     # Procedural sky & weather patterns
│       ├── visualiser.wgsl  # Audio-reactive frequency blooms
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
| `reqwest` | HTTP client for weather, LRCLIB, and iTunes fallback APIs |
| `serde` + `toml` | Config file serialisation |

## Prerequisites

- COSMIC desktop with `cosmic-comp` compositor
- PipeWire (standard on modern Linux)
- A media player that supports MPRIS (Spotify, VLC, Firefox, etc.)

### System Dependencies

To compile the application, you'll need the PipeWire development headers and Clang (required by `bindgen` to generate the PipeWire Rust bindings).

**Ubuntu / Pop!_OS:**
```bash
sudo apt install clang libclang-dev libpipewire-0.3-dev pkg-config
```
**Arch Linux:**
```bash
sudo pacman -S clang pipewire pkgconf
```

## Building

```bash
# Install Rust if you haven't
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/Kenyon-J/cosmic-wpengine
cd cosmic-wpengine
# This builds both the main engine and the cosmic-wallpaper-gui binary
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
    style: u32,
    time: f32,                 // Elapsed time in seconds for scrolling effects!
    pad1: u32,
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
- [ ] Generic video file playback (MP4/WebM) as backgrounds
- [ ] Plugin API for custom data sources

# cosmic-wallpaper

A live wallpaper engine for the [COSMIC desktop](https://system76.com/cosmic), written in Rust.

## Features (v1 roadmap)

- 🎵 **Album art wallpaper** — blurs and colour-grades your current album art as the background, updating automatically via MPRIS
- 📊 **Audio visualiser** — real-time frequency spectrum visualiser using PipeWire audio capture
- 🌤️ **Weather-reactive** — changes the scene based on current weather conditions via Open-Meteo
- 🌅 **Time of day** — ambient day/night cycle as a fallback scene
- ⚙️ **COSMIC settings integration** — configure everything from the COSMIC settings panel

## Architecture

```
cosmic-wallpaper/
├── src/
│   ├── main.rs              # Entry point, spawns subsystems
│   └── modules/
│       ├── config.rs        # Config loading (~/.config/cosmic-wallpaper/config.toml)
│       ├── state.rs         # Shared application state
│       ├── event.rs         # Event types between subsystems
│       ├── mpris.rs         # MPRIS music player watcher
│       ├── audio.rs         # PipeWire audio capture + FFT
│       ├── weather.rs       # Open-Meteo weather polling
│       ├── renderer.rs      # wgpu render loop
│       ├── wayland.rs       # Wayland layer surface (wlr-layer-shell)
│       └── colour.rs        # Album art colour extraction
└── src/shaders/
    ├── album_art.wgsl       # GPU shader: blurred album art + colour grade
    └── visualiser.wgsl      # GPU shader: frequency bar visualiser
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
latitude = 51.5   # your latitude
longitude = -0.1  # your longitude
poll_interval_minutes = 15

[audio]
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

- [ ] Multi-monitor support (one layer surface per output)
- [ ] COSMIC settings panel integration via libcosmic
- [ ] Shader-based particle effects (rain, snow, fire)
- [ ] D-Bus signal-based MPRIS (replace polling)
- [ ] Lyrics-synced visual pulses via LRCLIB
- [ ] User-loadable custom shaders

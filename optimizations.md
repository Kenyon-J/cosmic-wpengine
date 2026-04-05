# Memory Optimizations and Low-Latency Refactoring

## 1. Data Layout: Enum Boxing and Alignment

**Optimization:**
Boxed large variants in the `Event` enum (`ConfigUpdated`, `TrackChanged`, `WeatherUpdated`).
Boxed `MetadataUpdate` variant in the `MprisUpdate` enum.

**Why (Memory Savings):**
In Rust, an enum takes up space equal to its largest variant plus a discriminant. `Event` had very large variants (like `TrackInfo` and `Config` containing multiple `String`s and `Vec`s), meaning every single `Event` passed through the async channel was taking up massive amounts of stack space, even for small events like `PlaybackStopped`. By boxing the large variants (`Box<TrackInfo>`, `Box<Config>`, `Box<WeatherData>`), the size of `Event` shrinks to just `8 bytes (pointer) + discriminant`, dramatically reducing the memory footprint of the MPSC channels and reducing memcpy overhead during message passing. Same for `MprisUpdate::Metadata`.

**Code Snippet:**
```rust
pub enum Event {
    ConfigUpdated(Box<super::config::Config>),
    TrackChanged(Box<TrackInfo>),
    PlaybackStopped,
    // ...
    AudioFrame {
        bands: Box<[f32]>,
        waveform: Box<[f32]>,
    },
    WeatherUpdated(Box<WeatherData>),
}
```

## 2. Allocations: Boxed Slices for Fixed-Size Data

**Optimization:**
Replaced `Vec<f32>` with `Box<[f32]>` in `AudioFrame`, and will replace `Vec<T>` with `Box<[T]>` for `palette` and `lyrics` in `TrackInfo`, and `audio_bands`/`audio_waveform` in `AppState`. Replaced `String` with `Box<str>` in `TrackInfo` and `LyricLine`.

**Why (Memory Savings):**
A `Vec<T>` and `String` take up 24 bytes on the stack (pointer, length, capacity). For data that is created once and never resized (like the audio frames sent over the channel, or parsed track information), the `capacity` field is completely wasted space. Converting them to `Box<[T]>` and `Box<str>` drops the capacity field, reducing the stack size to 16 bytes (pointer, length) per collection. This improves data locality and shrinks struct sizes.

## 3. Data Layout: #[repr(transparent)]

**Optimization:**
Added `#[repr(transparent)]` to the newtype `PooledImage`.

**Why (Memory Savings):**
`PooledImage(Option<image::RgbaImage>)` is a newtype wrapper. Without `#[repr(transparent)]`, the compiler is free to add padding. With it, we guarantee that `PooledImage` has exactly the same memory layout and ABI as `Option<image::RgbaImage>`, ensuring zero-cost abstraction and preventing any potential layout bloat.

## 4. Async Runtime Efficiency

**Optimization:**
Wrapped `reqwest::Client::builder()` calls in `tokio::task::spawn_blocking` within async contexts (e.g., in `src/modules/weather.rs` and `src/modules/mpris.rs`).

**Why (Reduced CPU Stuttering):**
The `reqwest` client builder internally performs synchronous setup, including reading system TLS certificates from the filesystem. If done directly inside an `async fn`, it can stall the `tokio` executor, causing audio dropouts or visual hitches in the `wgpu` render loop. Wrapping it in a dedicated blocking thread ensures the async reactor remains responsive.

## 5. File I/O Optimization

**Optimization:**
Replaced synchronous `std::fs::read_to_string` with `tokio::fs::read_to_string().await` inside `VisualiserPass::load_shader_source`.

**Why (Reduced CPU Stuttering):**
Synchronous file reads inside an async function or the render loop will block the thread until the disk I/O completes. Using `tokio`'s async file system operations yields the execution context back to the runtime while waiting, maintaining a smooth framerate.

## 6. GPU Uniform Updates

**Optimization:**
Removed `last_uniform_res != Some(current_res)` checks gating uniform buffer writes in `draw_frame`.

**Why (Performance):**
Gating updates behind a memory comparison check in the highly animated render loop introduces redundant CPU overhead. Since uniforms like `time` and `audio_energy` change every frame anyway, updating the buffers unconditionally avoids branch mispredictions and the cost of the comparison itself.

## 7. Panic Prevention

**Optimization:**
Systematically replaced remaining `.unwrap()` calls with graceful error handling (e.g., `ok_or_else` or fallback logic) throughout the codebase.

**Why (Robustness):**
The wallpaper engine runs silently in the background. If a stray `.unwrap()` panics (due to missing metadata, failed allocations, or weird API responses), the entire process crashes, leaving the desktop black. Returning `Result` or providing sensible fallbacks guarantees uptime and stability.

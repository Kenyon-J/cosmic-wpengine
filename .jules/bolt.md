## 02-03-2025- Optimize DynamicImage Pixel Sampling
**Learning:** `DynamicImage::get_pixel` performs nested enum matching over 10+ variants and dynamic dispatch for every pixel lookup, which introduces significant overhead inside high-frequency nested pixel loops (e.g., color palette extraction, image averaging). Extracting the underlying `RgbaImage` reference once before the loop using `.as_rgba8()` / `.to_rgba8()` completely bypasses this dispatch.
**Action:** Always retrieve a direct reference to the underlying concrete image buffer (like `RgbaImage`) before performing pixel lookup operations in performance-sensitive loops.

## 03-03-2025- Bypass `ImageBuffer::get_pixel` Bounds Check and Offset Arithmetic
**Learning:** Even on concrete `ImageBuffer` (like `RgbaImage`), `.get_pixel(x, y)` performs coordinate bounds checks and dynamic offset multiplication `(y * width + x)` inside nested loops, which prevents LLVM from fully vectorizing. Extracting the flat subpixel slice `as_raw()` and precomputing `y * width` as a row offset in the outer loop completely avoids redundant coordinate arithmetic and bounds check assertions.
**Action:** For high-frequency image sampling or custom pixel-by-pixel loops on `ImageBuffer` types, index into the flat slice from `.as_raw()` directly using a pre-calculated row offset.

## 04-03-2025- Fast sRGB Interpolation and NaN-Safe Clamping
**Learning:** High-curvature functions like `x^(1/2.4)` have extremely steep derivatives near zero, causing significant linear interpolation errors on coarse grids. Increasing the interval count (e.g., from 256 to 1025) reduces the interpolation error by $O(h^2)$ (16x), well below visual and 8-bit precision limits, while keeping the lookup table inside L1 cache (4KB). Additionally, standard `f32::clamp` panics on `NaN` values, which must be safely bypassed using manual comparison checks.
**Action:** For high-frequency color or signal math, use fine-grained (1025-entry) linear-interpolated lookup tables to model high-curvature segments accurately, and always employ NaN-safe branch-free comparison clamping to prevent runtime panics on invalid values.

## 2025-03-05 - Avoid Redundant `wgpu` Buffer Writes in Hot Loops
**Learning:** Writing to GPU buffers via `wgpu::Queue::write_buffer` every frame, even when data hasn't changed, needlessly consumes PCIe bandwidth and CPU/GPU cycles.
**Action:** Always diff the incoming uniform/vertex/buffer data against a cached version (e.g., storing the last slice on the `Renderer` state) and only dispatch `write_buffer` if the data actually differs.

## 2025-03-05 - Box Large `tokio::sync::mpsc` Enums
**Learning:** Enums passed through async channels take up the size of their largest variant. `Event::AudioFrame` contained two heavy `PooledAudioBuffer` structures, bloating the entire `Event` footprint and wasting memory for simpler variants.
**Action:** When working with large enum variants traversing MPSC channels, box the large payloads (e.g., `AudioFrame(Box<(PooledAudioBuffer<f32>, PooledAudioBuffer<f32>)>)`) to drastically shrink the enum's stack/queue footprint.

## 2025-03-05 - Modern Standard Library Lazy Initialization
**Learning:** Relying on `lazy_static` or `once_cell` for simple static initializations requires adding external crate dependencies, which may fail compilation if missing from `Cargo.toml`.
**Action:** For simple static caches or lookup tables (like SRGB curves), use `std::sync::OnceLock::new()` and `.get_or_init()` available in standard Rust 1.70+ to avoid introducing unnecessary dependencies.

## 30-07-2026- Eliminate OnceLock and Closing-Closure Overhead inside High-Frequency Loops
**Learning:** Retrieving an initialized `OnceLock` reference (such as `get_linear_to_srgb_table()`) inside high-frequency nested loops (like gradient pixel-generation loops) introduces thread-safe atomic check overhead on every iteration. Passing the static reference directly to a parameter-based helper function (`linear_to_srgb_lut`) completely eliminates this check. Furthermore, constructing image pixel data directly via nested raw loop coordinate manipulation and flat `Vec` writes is significantly faster than using closure-based helper maps like `RgbaImage::from_fn`.
**Action:** Always hoist static/OnceLock lookups out of performance-sensitive inner loops, and prefer direct nested loop writing to pre-allocated buffers over closure-based mapping utilities.

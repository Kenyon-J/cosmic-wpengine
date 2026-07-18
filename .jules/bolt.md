# Bolt's Journal - cosmic-wallpaper Performance Optimizations

## 15-01-2025- Avoid DynamicImage enum dispatch overhead
**Learning:** Calling `image.get_pixel(x, y)` inside nested loops invokes a 10-variant enum matching overhead on `DynamicImage` on every single pixel access. By matching and getting a reference to `RgbaImage` once via `as_rgba8()` with a `to_rgba8()` fallback, we can bypass enum dispatch entirely.
**Action:** Always extract the underlying typed image reference (e.g. `RgbaImage`) before entering tight loops that read pixel data in image processing functions.

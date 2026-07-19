## 10-02-2025- Avoid Dynamic Enum Dispatch in Pixel Hot-Paths
**Learning:** Calling `image.get_pixel(x, y)` inside tight sampling loops on a `DynamicImage` incurs massive enum variant matching overhead for every pixel accessed. Resolving the enum variant to a concrete reference type (like `RgbaImage`) once before the loop completely avoids this overhead.
**Action:** Always obtain concrete references (e.g., via `as_rgba8()` or `to_rgba8()`) when reading thousands of pixels sequentially in sampling or K-means clustering functions.

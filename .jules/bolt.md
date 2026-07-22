## 02-03-2025- Optimize DynamicImage Pixel Sampling
**Learning:** `DynamicImage::get_pixel` performs nested enum matching over 10+ variants and dynamic dispatch for every pixel lookup, which introduces significant overhead inside high-frequency nested pixel loops (e.g., color palette extraction, image averaging). Extracting the underlying `RgbaImage` reference once before the loop using `.as_rgba8()` / `.to_rgba8()` completely bypasses this dispatch.
**Action:** Always retrieve a direct reference to the underlying concrete image buffer (like `RgbaImage`) before performing pixel lookup operations in performance-sensitive loops.

## 03-03-2025- Bypass `ImageBuffer::get_pixel` Bounds Check and Offset Arithmetic
**Learning:** Even on concrete `ImageBuffer` (like `RgbaImage`), `.get_pixel(x, y)` performs coordinate bounds checks and dynamic offset multiplication `(y * width + x)` inside nested loops, which prevents LLVM from fully vectorizing. Extracting the flat subpixel slice `as_raw()` and precomputing `y * width` as a row offset in the outer loop completely avoids redundant coordinate arithmetic and bounds check assertions.
**Action:** For high-frequency image sampling or custom pixel-by-pixel loops on `ImageBuffer` types, index into the flat slice from `.as_raw()` directly using a pre-calculated row offset.

## 03-02-2025- Optimize MPRIS module with Async (REJECTED)
**Learning:** For long-running, reactive tasks like MPRIS monitoring, a dedicated `std::thread` is often the better architectural choice. Moving to a polling model with `tokio::task::spawn_blocking` every few seconds introduces lag, increases overhead, and risks blocking the async executor if D-Bus calls are executed directly.
**Action:** Preserve dedicated threads for reactive event-based monitoring to ensure real-time responsiveness and avoid overloading the async task scheduler.

## 03-02-2025- Optimize audio visualiser with pre-calculated Hann window
**Learning:** Redundant trigonometric calculations in high-frequency hot loops (like audio DSP) can significantly increase CPU usage.
**Action:** Always pre-calculate static coefficients (like window functions) into lookup tables (arrays or vectors) outside the main processing loop to save thousands of redundant math operations per second.

## 04-02-2025- Offload Blocking Operations to Worker Threads
**Learning:** Mixing synchronous, CPU-intensive work (like image decoding) or blocking library calls (like D-Bus via the `mpris` crate) directly inside an async task stalls the Tokio executor, leading to frame drops and UI stutter.
**Action:** Always wrap heavy synchronous operations and blocking library calls in `tokio::task::spawn_blocking`. This offloads the work to a dedicated thread pool, preserving the responsiveness of the main async event loop.

## 02-04-2026- Optimize Bounded Histograms with Fixed-Size Arrays
**Learning:** For bounded counting tasks with a small key space (e.g., color histograms with 512 buckets), `HashMap` introduces unnecessary hashing overhead and heap allocations.
**Action:** Prefer fixed-size arrays over `HashMap` for performance-critical loops when the key space is small and can be mapped to indices efficiently.

## 05-02-2025- Optimize Multi-Monitor Rendering by Caching Monitor-Specific State
**Learning:** In multi-monitor environments, monitors often share identical resolutions and scale factors. Redundantly performing text shaping, vertex generation, and GPU uniform updates for every monitor consumes significant CPU/GPU time.
**Action:** Move display-invariant calculations (like font attributes and sky gradients) outside the per-monitor loop. Cache the resolution and scale factor of the previous monitor to skip redundant text preparation and GPU buffer writes if the current monitor configuration matches. Ensure all resources (e.g. text buffers) are correctly returned to their pools after the entire multi-monitor loop completes.

## 2025-05-18 - Fix MPRIS missing metadata when track ID is empty
**Learning:** Sometimes an MPRIS player (like some web browsers or lightweight players) will emit metadata but leave the `track_id` empty or default. If we only update metadata when `current_track_id != last_track_id`, consecutive tracks with empty IDs will be ignored.
**Action:** Always process the metadata update if the `current_track_id` is empty, ensuring that song changes without valid track IDs still propagate.

<<<<<<< bolt-video-memcpy-fastpath-16645586428494396973
## 2023-10-24 - Optimize densely packed image buffers with bulk memcpy
**Learning:** Looping row-by-row to copy image or video frames (e.g. `ffmpeg` or `image` crate buffers) introduces severe overhead from redundant bounds checks and pointer arithmetic. When `stride == width * channels` (densely packed buffers, common for RGBA video), this is completely unnecessary.
**Action:** Always check if `stride == expected_row_bytes`. If true, bypass the `for` loop and copy the entire frame in a single bulk operation using `copy_from_slice(&data[..frame_size])` to leverage highly optimized `memcpy` routines.
=======
## 23-05-2024- Optimize Staging Buffers and Hot-Loop Math
**Learning:** Re-using staging buffers (like `video_frame_buffer`) without calling `clear()` prevents redundant zero-filling by `Vec::resize()` in subsequent frames. Additionally, pre-calculating the reciprocal of viewport dimensions (`inv_width`, `inv_height`) outside of nested loops allows replacing multiple divisions with multiplications, which are significantly faster on most CPUs.
**Action:** Preserve capacity in scratch buffers between frames to avoid deallocations and reallocations. Use reciprocal multiplication for viewport normalization in performance-critical rendering loops.

## 07-02-2025- Cache Event-Driven State for the Hot Path
**Learning:** Performing O(N) calculations (like audio energy scanning) or complex conditional logic (like thematic text color derivation) in the main rendering loop is wasteful if the source data only changes on specific events. Caching these in the `Renderer` state reduces per-frame overhead.
**Action:** Identify display-invariant and frame-invariant properties derived from events. Move their calculation to the event handlers (`Event::AudioFrame`, `Event::TrackChanged`) and cache the results in the primary state struct for direct access during rendering.

## 11-02-2025- Optimize Multi-Monitor Rendering by Caching Monitor-Invariant Logic and Dimensions
**Learning:** Redundantly performing field accesses (like `texture.size()`) and enum-to-integer mappings inside a multi-monitor rendering loop wastefully consumes CPU cycles for every connected display.
**Action:** Cache all display-invariant properties (resolutions, mapped enums, and layout decisions) in the primary state or pre-calculate them once per frame before entering the per-monitor loop.

## 08-02-2025- Optimize Padded Buffer Copies with Iterators
**Learning:** When copying image data with row padding, manual indexing and slicing in a loop (e.g. `raw_rgba[src_start..src_end]`) prevents the compiler from fully optimizing the memory transfer. Using iterator-based patterns like `chunks_exact_mut().zip()` eliminates manual bounds checking and enables LLVM to apply auto-vectorization (SIMD).
**Action:** For all bulk memory copies involving stride or padding, prefer iterator-based `chunks_exact` and `zip` patterns over manual index arithmetic.
>>>>>>> master
## 2026-04-11 - Optimize Padded Buffer Copies with Iterators
**Learning:** When copying image data with row padding, manual indexing and slicing in a loop (e.g. `raw_rgba[src_start..src_end]`) prevents the compiler from fully optimizing the memory transfer. Using iterator-based patterns like `chunks_exact_mut().zip()` eliminates manual bounds checking and enables LLVM to apply auto-vectorization (SIMD).
**Action:** For all bulk memory copies involving stride or padding, prefer iterator-based `chunks_exact` and `zip` patterns over manual index arithmetic.

## 15-05-2026 - Optimize High-Frequency Hashing in Rendering Path
**Learning:** Repeatedly hashing the same strings (track info, weather, lyrics) inside a high-frequency rendering loop (60+ FPS) is a significant waste of CPU cycles, especially in multi-monitor setups where the loop executes multiple times per frame.
**Action:** Pre-calculate string hashes once in data-parsing modules or event handlers and store them alongside the data. In the hot path, use the cached `u64` hash instead of recalculating it from the string.

## 03-02-2025- Consolidate Frame-Invariant Calculations
**Learning:** In the rendering loop, certain values like sky colors and clear colors are display-invariant and frame-invariant. Calculating them multiple times (e.g., once for the clear color and again for shader uniforms) is wasteful.
**Action:** Move these calculations to a single point at the beginning of the frame, outside the monitor loop, and pass the results where needed.

## 10-02-2025- Cache O(N) Scene Detection Metrics
**Learning:** The `scene_description` logic was performing an O(N) sum over audio bands on every call. In the rendering hot path, especially with multiple monitors, this adds up to thousands of redundant iterations per second.
**Action:** Cache the average audio energy in `AppState` during the `Event::AudioFrame` handler, where the bands are already being iterated. This reduces `scene_description` to an O(1) field retrieval, eliminating redundant passes over the audio data.

## 2026-05-18 - Optimize non-cryptographic hashing
**Learning:** Using `std::collections::hash_map::DefaultHasher` (SipHash) inside high-frequency paths (like the 60FPS render loop's caching system) introduces measurable CPU overhead due to its cryptographic collision resistance.
**Action:** Always prefer `rustc_hash::FxHasher` for internal hashing and caching where HashDoS protection is unnecessary, as it provides significantly better performance.

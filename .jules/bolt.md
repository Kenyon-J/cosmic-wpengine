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

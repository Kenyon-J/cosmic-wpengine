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

## 2025-04-03 - Optimize Vector Creation in Hot Loops with iterators
**Learning:** Manual `for` loops appending array elements one by one via `.push()` can be up to 10-12x slower than standard library iterators because LLVM fails to vectorize the instructions.
**Action:** When mapping arrays and zipping items together, particularly in hot loops like audio DSP, always favor `.extend()` with `.zip()`, `.map()`, and `.chunks_exact()`. This allows LLVM to process data branchlessly and vectorize array handling.

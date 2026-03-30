## 03-02-2025- Optimize MPRIS module with Async (REJECTED)
**Learning:** For long-running, reactive tasks like MPRIS monitoring, a dedicated `std::thread` is often the better architectural choice. Moving to a polling model with `tokio::task::spawn_blocking` every few seconds introduces lag, increases overhead, and risks blocking the async executor if D-Bus calls are executed directly.
**Action:** Preserve dedicated threads for reactive event-based monitoring to ensure real-time responsiveness and avoid overloading the async task scheduler.

## 03-02-2025- Optimize audio visualiser with pre-calculated Hann window
**Learning:** Redundant trigonometric calculations in high-frequency hot loops (like audio DSP) can significantly increase CPU usage.
**Action:** Always pre-calculate static coefficients (like window functions) into lookup tables (arrays or vectors) outside the main processing loop to save thousands of redundant math operations per second.

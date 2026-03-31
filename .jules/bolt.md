## 2024-05-24 - [Async CPU Bottlenecks]
**Learning:** In Tokio async contexts in this codebase, CPU-intensive tasks like `image::load_from_memory` can block the worker threads, potentially stalling other subsystems like media polling or rendering.
**Action:** Always wrap heavy CPU operations like image decoding in `tokio::task::spawn_blocking`.

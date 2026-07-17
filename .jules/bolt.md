## 2024-05-24 - Optimize Cache Lookup using take()
**Learning:** In the `cosmic-wpengine` project, `Option<T>` cloning creates an unnecessary allocation for large data structures like `Box<[[f32; 3]]>` (the color palette).
**Action:** Use `.take()` to consume the `Option<T>` and pass ownership, avoiding redundant memory allocation. First verify it is safe to permanently consume the value and if its existence is still needed logic track it in an intermediate boolean (`is_some`).

## 2024-11-20 - Safe memory casting with bytemuck
**Learning:** `std::slice::from_raw_parts` is a heavy unsafe operation that the compiler can't easily optimize or verify. The project uses `bytemuck` which has safer casting for structs explicitly annotated with `#[derive(bytemuck::Pod, bytemuck::Zeroable)]` and `#[repr(C)]`.
**Action:** Replaced unsafe casting inside the hot render loop with safe `bytemuck::bytes_of` and `bytemuck::cast_slice` calls. Added necessary `bytemuck` traits to all types representing GPU memory buffers to allow safe and optimized bitwise casting.

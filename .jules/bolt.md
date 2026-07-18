## 2024-07-18 - Fix subnormal floats
**Learning:** In continuous decay physics (e.g., exponential smoothing or spring physics), failing to clamp variables to 0.0 when they become small (e.g., < `1e-5`) can cause float degradation into "subnormals", resulting in massive CPU slowdowns.
**Action:** Always check variables multiplied by fractional values for subnormals and clamp them to zero when they fall below a perceptible threshold.

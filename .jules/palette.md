## 2024-07-22 - Inline Validation
**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-11-20 - Inline Validation (Weather Coordinates)
**Learning:** Adding explicit validation and error styles (`.error()`) to form inputs (such as latitude/longitude) provides immediate inline feedback, avoiding silent failures or user confusion when saving invalid data.
**Action:** Always wrap `.error(...)` validation logically with `.is_empty()` checks to prevent showing errors on newly cleared fields, and use `.is_ok_and()` to concisely validate values.
## 2026-09-03 - Inline Validation (Theme Creation)
**Learning:** When replacing tooltip validation with inline validation using `.error()`, always explicitly check for both not empty and an existing issue so it only appears when actively typed.
**Action:** Always logically bind `.error()` inside an `if` block, e.g. `if !is_empty && already_exists { input = input.error(...) }`.

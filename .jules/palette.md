## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2024-08-08 - Inline Validation for Coordinate Inputs
**Learning:** Latitude and longitude inputs can fail silently if user types invalid numbers or numbers out of coordinate range, resulting in silent failures without UX indication.
**Action:** Use `.error()` on the `cosmic::widget::text_input` in line to indicate that a coordinate is not valid based on bounds (-90.0..=90.0) or parsing failure.

## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2026-08-24 - Inline Validation for Text Inputs
**Learning:** Silent failures in text inputs can be confusing and lead to poor user experience, particularly for constraints like latitude and longitude.
**Action:** Add inline validation by using the `.error()` method on text input widgets to provide immediate, actionable feedback when values are invalid.

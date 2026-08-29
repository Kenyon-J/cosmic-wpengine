## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-08-29 - Inline Validation for Latitude/Longitude Inputs

**Learning:** Text inputs that accept numerical values within a specific range (like latitude/longitude) should provide immediate visual feedback (inline validation) when the user enters an invalid or out-of-range value, rather than silently failing to update the configuration.
**Action:** Use `.error()` on the `text_input` widget when the parsed value is invalid, leaving empty inputs as valid to avoid aggressive error styling when clearing a field.

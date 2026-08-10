## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2024-08-10 - Conditional Error States for Forms
**Learning:** When implementing inline validation on forms (e.g., using `text_input(...).error(...)` in `cosmic::iced`), conditionally check if the input is empty first so that empty or newly cleared fields do not immediately display a validation error.
**Action:** Use conditional logic (e.g., `!is_empty && !is_valid`) when applying the `.error()` attribute to form inputs.

## 2024-07-22 - Inline Validation
**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2024-11-20 - Inline Validation (Weather Coordinates)
**Learning:** Adding explicit validation and error styles (`.error()`) to form inputs (such as latitude/longitude) provides immediate inline feedback, avoiding silent failures or user confusion when saving invalid data.
**Action:** Always wrap `.error(...)` validation logically with `.is_empty()` checks to prevent showing errors on newly cleared fields, and use `.is_ok_and()` to concisely validate values.

## 2025-05-15 - Inline Validation (Theme Name)
**Learning:** In the theme editor, when creating a new theme, silently disabling the 'Create' button or showing a tooltip when the theme name already exists can be less intuitive than showing an explicit red `.error()` style on the input field itself, which is standard for form validation.
**Action:** Use `.error()` on the `text_input` widget when a user tries to create a theme with a name that already exists, matching the pattern used in the weather location fields.

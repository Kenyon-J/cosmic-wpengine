## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2026-08-03 - Disabled States for Conditional Buttons
**Learning:** Buttons that require a prerequisite selection (like exporting a chosen item) should clearly indicate their disabled state with a tooltip when the prerequisite is not met, rather than failing silently or with a generic error upon click.
**Action:** Conditionally omit the `.on_press` handler and wrap the button in a `cosmic::widget::tooltip` to provide clear guidance to the user.

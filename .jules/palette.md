## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2024-07-28 - Tooltips on Disabled Buttons

**Learning:** When a button is disabled, users often don't know *why* it's disabled or what action they must take to enable it. Leaving a disabled state unexplained causes friction.
**Action:** Always wrap disabled buttons in a `cosmic::widget::tooltip` (or equivalent) providing clear instruction on how to unlock the action.

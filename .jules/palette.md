## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2026-07-26 - Adding loading icon to async states
**Learning:** Providing visual feedback during async operations like update checks prevents user confusion and multiple clicks.
**Action:** Add loading icons (like process-working-symbolic) to states that represent ongoing background work.

## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2026-08-02 - Async Loading States
**Learning:** Missing loading states for asynchronous operations like updates can leave users wondering if the app is frozen. In cosmic::iced, standard spinners are unavailable.
**Action:** Use the `process-working-symbolic` system icon alongside status text in a Row to provide visual feedback for asynchronous states.

## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-07-22 - Async Button Loading States
**Learning:** Buttons triggering async network operations (like "Use my location") lack visual feedback during loading, leading to uncertainty and potential duplicate clicks.
**Action:** Track async operations with a boolean state (`is_detecting_location`) and swap the standard text button out for a disabled icon button with a process-working-symbolic icon and a descriptive tooltip while active.

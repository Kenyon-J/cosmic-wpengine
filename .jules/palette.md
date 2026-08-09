## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-08-09 - Loading Indicators for Updates

**Learning:** Missing visual feedback for asynchronous operations (like checking for updates or downloading updates) can make the UI feel unresponsive and leave users wondering if their action registered.
**Action:** Always include a visual loading indicator, such as `cosmic::widget::icon::from_name("process-working-symbolic")`, alongside text feedback when a user triggers a network request or long-running task.

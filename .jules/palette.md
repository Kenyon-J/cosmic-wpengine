## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-07-25 - Async Visual Feedback

**Learning:** When performing long-running asynchronous background operations (like checking for updates or downloading files), relying solely on text feedback leaves the user unsure if the application is actually working or has frozen.
**Action:** Use standard system icons like `cosmic::widget::icon::from_name("process-working-symbolic")` alongside text to clearly communicate active background processes. Note that `cosmic::widget::spinner()` does not exist in this version of the `cosmic` UI library.

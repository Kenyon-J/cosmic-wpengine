## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-11-20 - Inline Validation (Weather Coordinates)
**Learning:** Adding explicit validation and error styles (`.error()`) to form inputs (such as latitude/longitude) provides immediate inline feedback, avoiding silent failures or user confusion when saving invalid data.
**Action:** Always wrap `.error(...)` validation logically with `.is_empty()` checks to prevent showing errors on newly cleared fields, and use `.is_ok_and()` to concisely validate values.
## 2024-11-20 - Update Feedback (Loading State)
**Learning:** In standard GUI applications, long-running asynchronous processes (like checking for updates or downloading files) can make the UI feel frozen or unresponsive if there is no explicit visual feedback.
**Action:** When creating updating or loading states in `cosmic::iced` where a native spinner is not available, insert `cosmic::widget::icon::from_name("process-working-symbolic")` into a `Row` to provide immediate, recognizable visual feedback to the user.

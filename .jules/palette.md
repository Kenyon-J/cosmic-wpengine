## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-11-20 - Inline Validation (Weather Coordinates)
**Learning:** Adding explicit validation and error styles (`.error()`) to form inputs (such as latitude/longitude) provides immediate inline feedback, avoiding silent failures or user confusion when saving invalid data.
**Action:** Always wrap `.error(...)` validation logically with `.is_empty()` checks to prevent showing errors on newly cleared fields, and use `.is_ok_and()` to concisely validate values.
## 2024-11-20 - Adding Loading States to Async Operations
**Learning:** For asynchronous operations such as checking for updates or installing updates, a static text message (e.g., "Checking for updates...") does not sufficiently indicate to the user that a background process is actively working. This can make the app feel frozen or unresponsive.
**Action:** Enhance text-only loading indicators by adding a visual spinner. In `cosmic::iced`, if a native spinner widget isn't available, use `cosmic::widget::icon::from_name("process-working-symbolic")` in a `Row` alongside the status text to provide clear visual feedback that the application is busy.

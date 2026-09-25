# Do not re-propose (reviewed 2026-09-25)

Check this list and the open PRs first - some of these were proposed 4-7 times.

**Already done:**
- Spinner + disabled state on "Use my location" (`detecting_location`, #568).
- Inline "name already exists" error on the new-theme input (#561).
- "Export pack" disabled with a tooltip until a theme is selected (#566).
- Inline validation on weather latitude/longitude (#514).
- Progress indicator for update check/install states (#535).

**Rejected:**
- Engine Start/Stop loading state derived by comparing `status_msg` to
  translated strings. If this is revisited, it needs a real state field
  (like `detecting_location`), not string comparison.

---

## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-11-20 - Inline Validation (Weather Coordinates)
**Learning:** Adding explicit validation and error styles (`.error()`) to form inputs (such as latitude/longitude) provides immediate inline feedback, avoiding silent failures or user confusion when saving invalid data.
**Action:** Always wrap `.error(...)` validation logically with `.is_empty()` checks to prevent showing errors on newly cleared fields, and use `.is_ok_and()` to concisely validate values.
## 2024-05-18 - Visual Feedback for Async Update States
**Learning:** In the `cosmic-wallpaper-gui` settings app, the `UpdateState::Checking` and `UpdateState::Updating` states previously only displayed static text, providing no visual indication that an asynchronous operation was occurring. This can make the UI feel frozen to the user.
**Action:** Used `cosmic::widget::icon::from_name("process-working-symbolic")` inside a `Row` alongside the text to provide a standard, animated visual indicator for these loading states, improving communication of system status without custom CSS or bloated dependencies.
## 2024-03-24 - Missing Localization Keys
**Learning:** When adding new UI text to `fl!()` macros (such as a tooltip or disabled state message), ensure the translation key is also added to the `.ftl` translation files to prevent runtime errors or missing text.
**Action:** Always `grep` the `.ftl` files to verify new translation keys exist, or update them accordingly.

## 2024-05-24 - Async Button Loading State
**Learning:** In cosmic::iced, users need visual feedback during async operations to prevent duplicate clicks and indicate progress, especially for network-bound actions like location detection.
**Action:** Replaced standard button with disabled custom button containing a loading spinner (icon `process-working-symbolic`) and text when `detecting_location` is true.

# Do not re-propose (reviewed 2026-09-25)

Check this list and the open PRs first - some of these were proposed 4-7 times.

**Already done:**
- Spinner + disabled state on "Use my location" (`detecting_location`, #568).
- Inline "name already exists" error on the new-theme input (#561).
- "Export pack" disabled with a tooltip until a theme is selected (#566).
- Inline validation on weather latitude/longitude (#514).
- Progress indicator for update check/install states (#535).
- Engine Start/Stop busy state via `starting_engine`/`stopping_engine`
  fields (#590), and a busy "Show" button while patch notes load (#579).
- Busy buttons share one helper, `view::busy_button(label, class)` - use it
  rather than hand-building another spinner row.

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

## 2024-05-24 - Async Button Loading States
**Learning:** Buttons triggering async network requests (like fetching patch notes) without visual loading states appear unresponsive and can cause user confusion or duplicate clicks.
**Action:** Always replace standard buttons with a disabled custom button containing a 'process-working-symbolic' icon while the async operation is in progress.

## 2024-05-24 - Async Engine Start/Stop State
**Learning:** In cosmic::iced, determining loading states by comparing `status_msg` against localized strings (via `fl!()`) is brittle and confusing to read. It's much cleaner and robust to manage transient async states with dedicated boolean fields in the Application struct.
**Action:** Replaced string-comparison logic with `starting_engine: bool` and `stopping_engine: bool` fields in `SettingsApp`, and updated the start/stop buttons to show `process-working-symbolic` spinners while these are true.

## 2024-10-06 - Prevent async layout shifts
**Learning:** In cosmic::iced, replacing standard buttons with raw text/spinner Rows during async operations (like update checking) causes abrupt layout and size shifts, making the UI feel jumpy.
**Action:** Always wrap transient async states in a matching disabled `button::custom` container (e.g. the app's `busy_button` helper) to preserve the original button's shape and dimensions during the network roundtrip.

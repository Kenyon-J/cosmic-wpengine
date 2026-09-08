## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-11-20 - Inline Validation (Weather Coordinates)
**Learning:** Adding explicit validation and error styles (`.error()`) to form inputs (such as latitude/longitude) provides immediate inline feedback, avoiding silent failures or user confusion when saving invalid data.
**Action:** Always wrap `.error(...)` validation logically with `.is_empty()` checks to prevent showing errors on newly cleared fields, and use `.is_ok_and()` to concisely validate values.
## 2024-05-18 - Visual Feedback for Async Update States
**Learning:** In the `cosmic-wallpaper-gui` settings app, the `UpdateState::Checking` and `UpdateState::Updating` states previously only displayed static text, providing no visual indication that an asynchronous operation was occurring. This can make the UI feel frozen to the user.
**Action:** Used `cosmic::widget::icon::from_name("process-working-symbolic")` inside a `Row` alongside the text to provide a standard, animated visual indicator for these loading states, improving communication of system status without custom CSS or bloated dependencies.
## 2024-11-20 - Custom Buttons in cosmic::iced
**Learning:** In `cosmic::iced`, when replacing a standard button with a `button::custom` (e.g., to create a disabled loading state containing a `Row` with an icon and text), append `.class(cosmic::theme::Button::Standard)` to the custom button to retain the standard visual styling.
**Action:** When creating a custom button with `button::custom`, chain `.class(...)` to specify the desired style, and ensure the resulting element is explicitly converted into a `cosmic::Element<'_, Message>` using `.into()`.
## 2024-11-20 - Initializing SettingsApp in cosmic-wallpaper-gui
**Learning:** In `cosmic-wallpaper-gui`, the `SettingsApp` struct is initialized inside the `impl Application for SettingsApp` block in `main.rs`. When adding new fields to the struct, ensure this specific initialization block is updated to prevent `E0063` compiler errors.
**Action:** Always search for `SettingsApp {` and specifically target the initialization block inside `impl Application` when adding new state variables.

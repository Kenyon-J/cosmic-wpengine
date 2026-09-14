## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-11-20 - Inline Validation (Weather Coordinates)
**Learning:** Adding explicit validation and error styles (`.error()`) to form inputs (such as latitude/longitude) provides immediate inline feedback, avoiding silent failures or user confusion when saving invalid data.
**Action:** Always wrap `.error(...)` validation logically with `.is_empty()` checks to prevent showing errors on newly cleared fields, and use `.is_ok_and()` to concisely validate values.
## 2024-05-18 - Visual Feedback for Async Update States
**Learning:** In the `cosmic-wallpaper-gui` settings app, the `UpdateState::Checking` and `UpdateState::Updating` states previously only displayed static text, providing no visual indication that an asynchronous operation was occurring. This can make the UI feel frozen to the user.
**Action:** Used `cosmic::widget::icon::from_name("process-working-symbolic")` inside a `Row` alongside the text to provide a standard, animated visual indicator for these loading states, improving communication of system status without custom CSS or bloated dependencies.
## 2024-05-19 - Disabling Buttons via Omitted Handlers
**Learning:** In `cosmic::iced`, the standard way to functionally disable an interactive widget (like a `button`) and prevent double-clicks during an asynchronous operation is simply to omit the `.on_press()` handler during its construction, rather than passing a separate `disabled=true` prop.
**Action:** When implementing loading or transitional states (e.g., 'Starting'/'Stopping' an engine), ensure the button is both visually distinct and functionally inactive by not assigning an `.on_press()` message.

## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2024-08-28 - Inline validation for text inputs
**Learning:** The text inputs for longitude and latitude did not previously have validation for bounds, only catching errors via checking valid bounds before calling 'schedule_debounced_save'. Adding `.error()` visual indicator for text inputs improves UX dramatically.
**Action:** Proactively calculate validity of longitude and latitude text fields in cosmic-wallpaper-gui/view.rs and conditionally apply the `.error` state to warn the user.

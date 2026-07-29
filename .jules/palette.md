## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-07-29 - Config folder UX
**Learning:** `xdg-open` on missing directories will silently fail.
**Action:** When invoking `xdg-open` from the app with standard UI buttons to show settings folders, ensure `std::fs::create_dir_all` is called first to guarantee it is there.

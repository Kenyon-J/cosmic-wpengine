## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2024-07-22 - Translation Key Localization
**Learning:** When adding a new translation key macro (like `fl!("status-select-theme-to-export")`) during UI enhancements, failing to also add the key string to the application's localization files (e.g., `i18n/en/io.github.kenyon_j.cosmic_wpengine.ftl`) will either cause a compile-time failure or display the unformatted key string directly to the user.
**Action:** Always manually insert newly introduced translation keys into the corresponding `.ftl` localization files.

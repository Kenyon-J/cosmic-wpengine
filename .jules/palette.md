## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2026-08-19 - Weather Input Validation
**Learning:** Silent failures when user input doesn't meet unstated constraints (like valid latitude ranges) leads to confusing UX where the application silently drops saves.
**Action:** Proactively apply inline  validation to fields such as latitude and longitude to intercept out-of-bounds input visually instead of waiting for a backend failure.
## 2024-08-01 - Weather Input Validation
**Learning:** Silent failures when user input doesn't meet unstated constraints (like valid latitude ranges) leads to confusing UX where the application silently drops saves.
**Action:** Proactively apply inline `.error()` validation to fields such as latitude and longitude to intercept out-of-bounds input visually instead of waiting for a backend failure.

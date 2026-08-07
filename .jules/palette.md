## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.

## 2024-08-07 - Loading States

**Learning:** When performing asynchronous actions like checking for or downloading updates, a static text message (e.g., "Checking...") provides poor visual feedback and can make the app appear frozen.
**Action:** Enhance text-based loading states by wrapping them in a `Row` and prepending a standard system spinner icon (e.g., `cosmic::widget::icon::from_name("process-working-symbolic")`).

## 2024-07-22 - Inline Validation

**Learning:** Forms without inline validation or descriptive disabled states can lead to confusing user experiences and silent failures.
**Action:** Proactively calculate validity and conditionally apply widget methods (e.g., `.on_press`, `.on_submit`) to avoid silent errors. Provide a descriptive `cosmic::widget::tooltip` when elements are disabled.
## 2025-01-30 - Conditional Widget Types in Layouts

**Learning:** When conditionally rendering different widget types or states (e.g., an active button versus a tooltip-wrapped disabled button) inside a layout method like `Row::new().push(...)`, the compiler will fail with "type annotations needed" or trait bound errors because the branches yield different specific types (like `button::Builder` vs `Tooltip`).
**Action:** Explicitly assign the widget to a typed variable `let el: cosmic::Element<'_, Message> = widget.into();` or call `.into()` on each branch *before* it is returned to the `push` call to ensure a unified `Element` type.

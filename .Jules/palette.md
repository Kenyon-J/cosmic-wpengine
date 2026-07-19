## 2024-07-19 - Conditional Disabled State in Iced
**Learning:** In the `cosmic::iced` framework, disabling a button visually and functionally is achieved by conditionally omitting the `.on_press()` event handler, rather than setting a specific `disabled` attribute or property.
**Action:** Always check the text input state and conditionally append `.on_press()` only when the input is valid/non-empty.

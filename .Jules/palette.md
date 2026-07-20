## 2024-06-25 - Placeholder and Validation Tooltips
**Learning:** In cosmic::iced, disabled buttons need explicit tooltips and conditional `.on_press()` logic to effectively communicate why the action is disabled. It's also important to use the first argument of `text_input` for dynamic placeholders.
**Action:** Use conditional rendering and `cosmic::iced::widget::tooltip` wrapping to explain why buttons are disabled instead of just omitting `.on_press()` silently, and always guide inputs with helpful placeholders.

## 2024-04-08 - Iced Tooltip Component Implementation
**Learning:** Adding accessibility tooltips to `cosmic::iced::widget` form elements is extremely straightforward by wrapping the element in `cosmic::iced::widget::tooltip(widget, "text", position)`. Because `tooltip` delegates state and interaction back to the wrapped element seamlessly, it can be retrofitted across the UI without breaking existing layout chains, state management logic, or event bindings.
**Action:** When adding UX tooltips to cosmic-wallpaper settings, proactively wrap isolated configurations (like `checkbox` and `pick_list`) using this method, especially where labels alone lack full context for new users.

## 2026-04-12 - Symmetrical Tooltips for Conditionally Disabled Buttons
**Learning:** When conditionally disabling buttons in `cosmic::iced` (by omitting `.on_press()`), providing a tooltip only for the disabled state leads to an inconsistent user experience. When the button becomes enabled, the contextual help disappears, right when the user can actually interact with it.
**Action:** When adding tooltips to conditionally disabled buttons, ensure tooltips are provided for both the enabled and disabled states to maintain symmetrical UX. This preserves helpful context regardless of the element's interactability.

## 2024-04-08 - Iced Tooltip Component Implementation
**Learning:** Adding accessibility tooltips to `cosmic::iced::widget` form elements is extremely straightforward by wrapping the element in `cosmic::iced::widget::tooltip(widget, "text", position)`. Because `tooltip` delegates state and interaction back to the wrapped element seamlessly, it can be retrofitted across the UI without breaking existing layout chains, state management logic, or event bindings.
**Action:** When adding UX tooltips to cosmic-wallpaper settings, proactively wrap isolated configurations (like `checkbox` and `pick_list`) using this method, especially where labels alone lack full context for new users.
## 2024-04-10 - Consistent Tooltips for Conditional Controls
**Learning:** Conditionally disabled buttons often have tooltips explaining *why* they are disabled, but lose their tooltip when enabled. This asymmetric behavior is confusing and makes the interface less discoverable.
**Action:** When adding a tooltip to a button's disabled state (by omitting `.on_press()`), always add a corresponding tooltip to its enabled state to maintain symmetry and ensure the component is consistently communicative.

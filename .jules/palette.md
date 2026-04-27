## 2024-04-08 - Iced Tooltip Component Implementation
**Learning:** Adding accessibility tooltips to `cosmic::iced::widget` form elements is extremely straightforward by wrapping the element in `cosmic::iced::widget::tooltip(widget, "text", position)`. Because `tooltip` delegates state and interaction back to the wrapped element seamlessly, it can be retrofitted across the UI without breaking existing layout chains, state management logic, or event bindings.
**Action:** When adding UX tooltips to cosmic-wallpaper settings, proactively wrap isolated configurations (like `checkbox` and `pick_list`) using this method, especially where labels alone lack full context for new users.
## 2026-04-17 - Added loading state to Patch Notes button
**Learning:** Providing explicit feedback and disabling buttons during async network operations prevents multiple concurrent requests and improves user confidence.
**Action:** Always consider the loading state for buttons that trigger network requests.
## 2024-04-18 - pick_list Placeholders
**Learning:** `pick_list` components in `cosmic::iced` can appear entirely blank and broken if no default value is provided or the list is empty, which severely degrades UX and clarity.
**Action:** Always append `.placeholder("Select...")` to `pick_list` components to ensure a descriptive empty state is shown.
## 2024-04-27 - Actionable Empty States
**Learning:** Generic "Not found" messages in UI placeholders or empty states cause friction because users don't know *how* to resolve them. For example, telling a user "No videos found" leaves them wondering where to put videos to make them appear.
**Action:** Always prefer providing an actionable path, instruction, or call-to-action in empty states (e.g., "Place videos in ~/.config/cosmic-wallpaper/videos") so users can immediately resolve the missing data state.

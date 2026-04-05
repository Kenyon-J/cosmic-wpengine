## 2024-04-01 - Visual Feedback via Event Handlers in cosmic::iced
**Learning:** In the `cosmic::iced` GUI framework, interactive elements like buttons do not use a separate "disabled" boolean property or custom CSS classes for styling inactive states. Instead, the framework infers the interactive state directly from the presence of an event handler. When `.on_press()` is omitted, the button automatically transitions into a visually disabled state (reduced opacity, no hover effects).
**Action:** When implementing forms or toolbars in libcosmic applications, always conditionally chain `.on_press()` handlers based on application state (e.g., input validation, selection availability) rather than relying on manual state checks inside the handler itself. This ensures users receive immediate visual feedback about which actions are currently available.

## 2024-04-05 - Clickable Checkbox Labels in COSMIC App
**Learning:** Using separate text widgets next to a checkbox (e.g., `row().push(checkbox(...)).push(text(...))`) reduces the clickable target area solely to the tiny checkbox square, negatively impacting usability and accessibility (violating Fitts's Law).
**Action:** When building UI using the `cosmic::iced` design system, always use the built-in `.label("Text")` modifier on the `checkbox` widget itself to ensure the label acts as a valid, grouped, and extended click target.

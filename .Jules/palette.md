## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-26 - Conditional Text Input Submission
**Learning:** In `cosmic::iced`, simply leaving a text input active while disabling its corresponding submission button creates a poor user experience, as users can still submit empty forms via the Enter key if `.on_submit()` is always attached.
**Action:** To correctly prevent form submission from empty or invalid inputs, extract the base `text_input` into a mutable variable, conditionally apply the `.on_submit()` method only if validation passes (e.g., `if !text.trim().is_empty()`), and then yield the input variable.
## 2024-05-31 - Read-only State for Text Editor
**Learning:** In `cosmic::iced`, simply leaving a `text_editor` fully active when there is no data to save (e.g., when no file is selected) causes confusion as users can type but their changes will be lost.
**Action:** To make a `text_editor` read-only, conditionally apply the `.on_action()` method. Omitting `.on_action()` prevents the editor from responding to keyboard input, and wrapping the disabled state in a tooltip makes the reason clear to the user.

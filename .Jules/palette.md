## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-26 - Conditional Text Input Submission
**Learning:** In `cosmic::iced`, simply leaving a text input active while disabling its corresponding submission button creates a poor user experience, as users can still submit empty forms via the Enter key if `.on_submit()` is always attached.
**Action:** To correctly prevent form submission from empty or invalid inputs, extract the base `text_input` into a mutable variable, conditionally apply the `.on_submit()` method only if validation passes (e.g., `if !text.trim().is_empty()`), and then yield the input variable.

## 2024-05-30 - Conditional Text Editor Read-Only State
**Learning:** In `cosmic::iced`, if a `text_editor` is always active but lacks a save mechanism (e.g., when no file is selected), it creates a confusing UX where users can type into a buffer they cannot save, violating the expectation that editable text implies actionable persistence.
**Action:** To make a `text_editor` read-only until it is valid to edit (like when a file is selected), extract the base editor to a mutable variable and conditionally apply `.on_action()` (e.g., `if is_file_selected { editor = editor.on_action(...) }`). Omitting `.on_action()` safely disables text input.

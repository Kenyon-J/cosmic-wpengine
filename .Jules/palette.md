## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-26 - Conditional Text Input Submission
**Learning:** In `cosmic::iced`, simply leaving a text input active while disabling its corresponding submission button creates a poor user experience, as users can still submit empty forms via the Enter key if `.on_submit()` is always attached.
**Action:** To correctly prevent form submission from empty or invalid inputs, extract the base `text_input` into a mutable variable, conditionally apply the `.on_submit()` method only if validation passes (e.g., `if !text.trim().is_empty()`), and then yield the input variable.

## 2024-06-04 - Editor Disabled Empty States
**Learning:** In `cosmic::iced`, simply leaving a `text_editor` widget active when no file is selected allows users to type into an empty void, creating a confusing experience where edits are inevitably lost.
**Action:** Always conditionally disable `text_editor` widgets by omitting `.on_action()` when their backing data source is unavailable, and provide a context-specific `.placeholder()` explaining why the editor is currently inactive (e.g., "Select a file to start editing").

## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-26 - Conditional Text Input Submission
**Learning:** In `cosmic::iced`, simply leaving a text input active while disabling its corresponding submission button creates a poor user experience, as users can still submit empty forms via the Enter key if `.on_submit()` is always attached.
**Action:** To correctly prevent form submission from empty or invalid inputs, extract the base `text_input` into a mutable variable, conditionally apply the `.on_submit()` method only if validation passes (e.g., `if !text.trim().is_empty()`), and then yield the input variable.

## 2024-06-03 - Conditional Editor Interactivity and Placeholders
**Learning:** In `cosmic::iced`, simply displaying a disabled `text_editor` element when it should be inactive can lead to a confusing user experience because the empty block lacks context.
**Action:** When a `text_editor` might be rendered in a disabled state (e.g. no file is selected), use `.placeholder("message")` to explain to the user why the editor is inactive or what needs to be done. To completely disable the editor, conditionally apply the `.on_action()` method to it only when the editor should be editable.

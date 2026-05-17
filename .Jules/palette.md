## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-19 - Conditional Submission in TextInputs
**Learning:** In `cosmic::iced`, if a text input shouldn't allow empty submissions (e.g., when creating a new theme name), we can dynamically attach the `.on_submit` method only when the current input passes validation. This prevents users from triggering an action with invalid data.
**Action:** Extract the base text input into a mutable variable (`let mut theme_input = text_input(...)`), and use an `if` block to attach `.on_submit` conditionally (`theme_input = theme_input.on_submit(...)`) before yielding it to the layout.

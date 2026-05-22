## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-19 - Conditional Submission for Text Inputs
**Learning:** In `cosmic::iced`, if a text input's `.on_submit` event requires valid data to function properly (e.g. creating a theme name), failing to validate or dynamically attach the submit event allows users to submit empty strings via the "Enter" key. Simply verifying in the message handler without UI feedback leads to silent failures.
**Action:** When validating a `text_input` in `iced`, assign the base input to a mutable variable, conditionally apply `.on_submit` only if the state passes validation, and provide explicit user feedback via a status message if invalid submissions are somehow triggered.

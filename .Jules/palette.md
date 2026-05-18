## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2025-05-18 - Conditional Form Submission for Empty Text Inputs
**Learning:** In `cosmic::iced`, attaching an unconditional `.on_submit` to a `text_input` can allow users to trigger actions (like theme creation) with empty or whitespace-only strings. If the backend blocks empty strings, the submission silently fails without providing inline feedback or preventing the enter key press.
**Action:** When a text input requires valid data before submission, assign the base `text_input` to a variable and conditionally apply `.on_submit` only if the input meets validation criteria (e.g., `!text.trim().is_empty()`). This naturally disables the "Enter" key trigger when invalid.

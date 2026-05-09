## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-19 - Conditionally Disabling Text Input Submission
**Learning:** In `cosmic::iced`, conditionally disabling form submission on a button is insufficient if the associated `text_input` can still submit the action via the Enter key. This allows users to bypass validation (e.g., submitting an empty string).
**Action:** Always conditionally apply the `.on_submit` method to `text_input` widgets to maintain validation parity with their corresponding disabled submit buttons.

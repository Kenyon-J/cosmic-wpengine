## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-11-20 - Prevent Empty Form Submissions with Inline Feedback
**Learning:** Allowing empty form submissions (even those consisting of only whitespace) leads to a poor user experience, as it either silently fails or creates unintended empty resources.
**Action:** Always validate form inputs (e.g., using `.trim().is_empty()`). Conditionally disable submission handlers on form elements (like `text_input.on_submit()`) if validation fails, and provide clear inline feedback (e.g., via a status message) when the user attempts the action.

## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-19 - Conditionally Disabling Text Input Submission in Iced
**Learning:** In `cosmic::iced`, conditionally disabling the `on_submit` behavior of a `text_input` (e.g., to prevent users from submitting empty strings) requires assigning the base input widget to a mutable variable, checking the condition, and then conditionally chaining the `.on_submit()` method before returning the widget.
**Action:** When creating a `text_input` that should not submit invalid state, assign the input to a mutable variable, conditionally apply `.on_submit()` if validation passes, and then yield the variable, avoiding jarring hidden elements or silent submission failures.

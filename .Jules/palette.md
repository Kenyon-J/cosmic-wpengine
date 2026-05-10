## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-20 - Adding conditional tooltips on disabled interactive elements
**Learning:** Providing tooltips on conditionally disabled interactive elements helps inform the user why they are disabled. However, if using iced 0.14+ returning tooltips vs returning standard elements requires uniform type casting into the root `Element` instance to avoid `match arms have incompatible types` compilation errors.
**Action:** Make sure to assign the conditional elements to a properly typed `let element: cosmic::Element<'_, super::Message> = ...` and ensure all branches return `Element` consistently.

## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-20 - Conditionally Hiding Irrelevant Settings
**Learning:** In cosmic::iced, keeping irrelevant settings visible with explanatory tooltips creates visual clutter. It is better to conditionally hide a setting (like a slider) entirely when it does not apply to the currently selected mode.
**Action:** Use a mutable container (like `let mut main_col = column()...`) to conditionally push configuration rows to the UI based on top-level state selections.

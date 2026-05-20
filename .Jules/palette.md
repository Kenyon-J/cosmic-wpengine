## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-19 - Vertical Alignment for Iced Rows
**Learning:** In `cosmic::iced`, UI elements of varying heights within a `row()` can appear misaligned or cause jarring layout shifts. The framework does not support `.align_items()` for `Row` widgets.
**Action:** Use `.align_y(cosmic::iced::Alignment::Center)` on the `row()` to gracefully and consistently vertically center elements of differing heights, improving visual polish and alignment.

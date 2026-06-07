## 2024-05-19 - Explicit Type Signatures for Iced Conditional Elements
**Learning:** When conditionally rendering elements in `cosmic::iced` with differing `.into()` traits (e.g., branching between an active widget with `.on_toggle()` and a disabled widget without it), the Rust compiler can fail to infer the exact type of the unified `Element`, resulting in `E0283: type annotations needed`.
**Action:** When creating conditionally configured UI branches in `iced` that require coercion into a generic `Element` container, assign the `if/else` block to a strictly typed intermediate variable (e.g., `let element: cosmic::Element<'_, super::Message> = if ... { ... };`) to guide the compiler's type inference.

## 2024-05-26 - Conditional Text Input Submission
**Learning:** In `cosmic::iced`, simply leaving a text input active while disabling its corresponding submission button creates a poor user experience, as users can still submit empty forms via the Enter key if `.on_submit()` is always attached.
**Action:** To correctly prevent form submission from empty or invalid inputs, extract the base `text_input` into a mutable variable, conditionally apply the `.on_submit()` method only if validation passes (e.g., `if !text.trim().is_empty()`), and then yield the input variable.
## 2024-06-03 - Contextual Guidance for Text Editors
**Learning:** In `cosmic::iced`, leaving a `text_editor` completely blank without placeholder text creates ambiguity, especially in multi-functional interfaces. Users may not know if the editor is broken, loading, or simply waiting for input. The `.placeholder()` method can accept dynamically generated strings based on application state.
**Action:** When creating text editors that rely on external state (like a selected file), use dynamic placeholders (e.g., `if state.is_some() { "Type here..." } else { "Select a file..." }`) to provide immediate, contextual guidance without needing separate UI labels.
## 2024-06-10 - Read-Only Text Editors
**Learning:** In `cosmic::iced`, a `text_editor` widget is active and editable by default. When the editor is used to display content that cannot be saved (e.g., when no file is selected, or when showing static info), leaving it editable allows users to type into a void, causing confusion.
**Action:** To make a `text_editor` read-only, conditionally apply the `.on_action()` method. If `.on_action()` is omitted, the editor becomes read-only and ignores keyboard input, preventing users from attempting to edit non-saveable text.

## 2024-06-12 - Vertical Alignment in cosmic::iced Rows
**Learning:** In `cosmic::iced`, elements within a `row()` that have differing heights (e.g., text labels next to pick_lists or buttons) are aligned to the top by default, causing a jagged visual appearance. Furthermore, `.align_items()` is not a valid method for `Row` in this version of the framework.
**Action:** To vertically center elements of varying heights within a `row()`, always use the `.align_y(cosmic::iced::Alignment::Center)` method to ensure a clean, balanced layout.

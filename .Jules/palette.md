
## 2024-05-18 - Conditional Form Validation Tooltips
**Learning:** In `cosmic::iced`, conditionally disabling form elements (by omitting `.on_press` or `.on_submit`) provides no visual feedback as to *why* the action is disabled, leading to user confusion. Forms like "Create new theme" can silently fail if validation fails.
**Action:** When disabling interactive elements, calculate validation state locally, construct the widgets conditionally, and wrap the action button in a `cosmic::iced::widget::tooltip` explaining the failure reason.

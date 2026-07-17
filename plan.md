1. Update `src/modules/gui/mod.rs` using the `replace_with_git_merge_diff` tool to track an `is_dirty: bool` state in `SettingsApp`. It will be set to `true` upon `EditorAction::Edit` and reset to `false` when files are saved or loaded.
2. Update `src/modules/gui/view.rs` using the `replace_with_git_merge_diff` tool to visually disable the Save button when `!app.is_dirty` by removing `.on_press`, append a `*` to indicate unsaved changes, and update tooltips.
3. Append a journal entry to `.Jules/palette.md` using the `run_in_bash_session` tool with `cat << 'EOF' >>` to record this reusable UX and performance pattern for `cosmic::iced` editors.
4. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
5. Use the `submit` tool to create the PR for this UX improvement.

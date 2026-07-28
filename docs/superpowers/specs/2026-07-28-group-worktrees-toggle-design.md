# Group Worktrees Toggle Design

## Goal

Let users choose whether worktrees appear as top-level rows or nested under their project in the Tab Focused and Agents Focused sidebars.

## Behavior

- Add a persisted **Group worktrees** toggle to **Settings → Layout**.
- Show the toggle while **Tab Focused** or **Agents Focused** is selected.
- Default the toggle to off so existing top-level worktree behavior remains unchanged.
- When off in Tab Focused, show each project followed by its non-primary worktrees that have open tabs.
- When off in Agents Focused, preserve its current project and top-level worktree rows.
- When on, show project rows only and render worktrees inside each expanded project in both layouts.
- Preserve each layout's existing tab lists and row actions inside grouped worktrees.
- Apply changes immediately without restarting Muxy.

## Architecture

Store the Boolean preference alongside existing worktree-list preferences and bind it with `@AppStorage` in both the settings view and shared sidebar. Keep row selection in a small pure resolver so grouped and ungrouped behavior can be tested without rendering SwiftUI.

Register the preference in `SettingsCatalog` so it participates in settings search and `settings.json` synchronization. Reuse the existing `TabFocusedWorktreeTree`, `WorktreeLeafRow`, and `TabFocusedSidebarRowItem` implementations, passing the sidebar content through the grouped tree so it renders the correct tab list and actions for each layout.

## Testing

- Verify grouped Tab Focused rows contain projects only.
- Verify ungrouped Tab Focused rows contain projects plus non-primary worktrees with open tabs.
- Verify grouped Agents Focused rows contain projects only.
- Verify ungrouped Agents Focused rows preserve its current top-level worktrees.
- Verify grouped worktree rows retain the correct content for each layout.
- Verify the preference is registered and searchable in the settings catalog.
- Run `scripts/checks.sh --fix`.

## Documentation

Document the toggle, its visibility for Tab Focused and Agents Focused, its default, and both sidebar presentations in the settings guide.

# S07 Layouts and worktrees

A project's `.muxy/layouts/` file builds split terminals on request, and Git
worktrees become child projects, from the CLI and from the app.

Start: run `$E g $VM reset`. The `alpha` fixture has `.muxy/layouts/trio.yaml`:
a left pane, and on the right a top and a bottom pane; each prints
`layout-<left|top|bottom>-42`. Tests continue from each other in order.

## T07.1 Apply a layout

1. Press add project `$E press $VM add-project`, `$E type $VM '~/e2e/alpha'`,
   `sleep 1`, `$E key $VM Return`, then `$E g $VM wait 'muxy@'`.
2. **Shot** `t07-1-before`. Check: the title bar's right-hand group has an icon that
   looks like a 2×2 grid of squares ("Apply Layout"). It is shown only for projects
   that have layouts.
3. Click it: `$E click-element $VM 'Apply Layout'`.
4. **Shot** `t07-1-picker`. Check: a list with one row `trio` and
   `.muxy/layouts/trio.yaml`. If instead you see something else, press
   `$E key $VM Escape` and mark `INSTRUCTION` with the shot.
5. `$E key $VM Return`.
6. **Shot** `t07-1-confirm`. Check: a dialog "Apply Layout?" naming `trio` and
   `alpha`, with "Apply Layout" and "Cancel".
7. `$E key $VM Return`. Then `$E g $VM wait 'layout-bottom-42'`.
8. Run `$E g $VM find 'layout-'`. Check: three lines, ending in
   `layout-left-42`, `layout-top-42` and `layout-bottom-42`, with three different ids.
9. **Shot** `t07-1-applied`. Check: one tab named `left`, with three panes: a
   full-height left pane and two stacked right panes, showing those outputs.

## T07.2 A worktree created by the CLI

1. Run `$E g $VM muxy worktree create alpha feat-cli`. It prints the new project.
2. Run `$E g $VM muxy worktree list alpha`. Check: a `feat-cli` line with a folder
   ending in `/worktrees/alpha/feat-cli`.
3. Press sidebar toggle `$E press $VM sidebar` to expand the sidebar.
4. **Shot** `t07-2-sidebar`. Check: an `alpha` row is in the sidebar.
5. Right-click the `alpha` row: `$E click-element $VM alpha --right`
   (it right-clicks the row found by that name).
6. **Shot** `t07-2-menu`. Check: a menu with "New Terminal Tab", "Existing
   Terminals…", "Apply Layout…", "Rename…", "Worktrees", "Remove Project…".
7. Open "Worktrees": `$E click-element $VM Worktrees`. **Shot** `t07-2-submenu`. Check:
   a submenu with "Show Worktrees". Click it: `$E click-element $VM 'Show Worktrees'`.
8. **Shot** `t07-2-chevron`. Check: the `alpha` row now ends with a small `>`
   chevron (its worktrees are collapsed). Click it:
   `$E click-element $VM 'Expand Worktrees'`.
9. **Shot** `t07-2-shown`. Check: under `alpha`, rows `alpha` (marked PRIMARY)
   and `feat-cli`, and a "New Worktree" row.

## T07.3 A worktree created in the app

1. Press palette `$E press $VM palette`, `$E type $VM 'new worktree'`.
2. **Shot** `t07-3-palette`. Check: "New Worktree…" is listed and not greyed out.
   `$E key $VM Return`.
3. **Shot** `t07-3-dialog`. Check: a "New Worktree" dialog with a Name field,
   "Create new branch", "Base branch" `main`, and "Create".
4. `$E type $VM 'feat-ui'`, then `$E key $VM Return`. Wait: `sleep 4`.
5. **Shot** `t07-3-created`. Check: the sidebar shows `feat-ui` under `alpha`,
   selected; the main area says "No tabs in feat-ui"; the status bar shows the
   branch `feat-ui`.
6. Run `$E g $VM muxy worktree list alpha`. Check: a `feat-ui` line.
7. Press new tab `$E press $VM new-tab`, wait `sleep 3` (other panes already
   show `muxy@`, so don't wait for the prompt here),
   `$E type $VM 'echo "wt=$(git branch --show-current)"'`, `$E key $VM Return`,
   `$E g $VM wait 'wt=feat'`. Check: it prints `wt=feat-ui`.

## T07.4 Remove a worktree from the CLI

1. Run `$E g $VM muxy worktree remove feat-cli --yes`.
2. Wait: `sleep 2`. **Shot** `t07-4-removed`. Check: `feat-cli` is gone from the
   sidebar; `alpha` and `feat-ui` remain.
3. Run `$E g $VM lines '~/.muxy/worktrees/alpha/feat-cli/README.md'`. Check: `0`
   (its folder was removed).

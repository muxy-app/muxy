# S03 Tabs and panes

Splitting, moving focus, zooming, closing (with and without a running
program), switching and renaming tabs, and detaching a terminal.

Start: run `$E g $VM reset`. Tests continue from each other in order.

## T03.1 Split right

1. Press new tab `$E press $VM new-tab`, then `$E g $VM wait 'muxy@'`.
2. Press split right `$E press $VM split-right`. Wait: `sleep 2`.
3. `$E type $VM 'echo right-$((1+1))'`, `$E key $VM Return`, `$E g $VM wait 'right-2'`.
4. **Shot** `t03-1-split`. Check: two panes side by side; `right-2` is in the
   right pane, which is focused: its header shows action icons on the right,
   and the left pane looks dimmed.
5. Run `$E g $VM muxy session list`. Check: two lines (two terminals).

## T03.2 Split down

1. Press split down `$E press $VM split-down`. Wait: `sleep 2`.
2. `$E type $VM 'echo lower-$((2+2))'`, `$E key $VM Return`, `$E g $VM wait 'lower-4'`.
3. **Shot** `t03-2-split`. Check: three panes: a full-height left pane, and on
   the right two panes stacked; `lower-4` is in the bottom-right pane.

## T03.3 Move focus

1. Press focus left `$E press $VM focus-left`.
2. `$E type $VM 'echo left-$((3+3))'`, `$E key $VM Return`, `$E g $VM wait 'left-6'`.
3. **Shot** `t03-3-focus`. Check: `left-6` is in the left pane, and the left
   pane is focused: its header shows action icons and it isn't dimmed.

## T03.4 Zoom a pane

1. Press zoom `$E press $VM zoom`.
2. **Shot** `t03-4-zoomed`. Check: only one pane is visible, filling the
   terminal area, and it shows `left-6`.
3. Press zoom again (same key).
4. **Shot** `t03-4-unzoomed`. Check: all three panes are visible again.

## T03.5 Close an idle pane

1. Press focus right `$E press $VM focus-right`.
2. Press close pane `$E press $VM close-pane`. No dialog may appear.
3. **Shot** `t03-5-closed`. Check: two panes remain and no dialog is shown.
4. Run `$E g $VM muxy session list`. Check: two lines.

## T03.6 Close confirmation for a running program

1. `$E type $VM 'sleep 600'`, `$E key $VM Return`. (Type into the focused pane.)
2. Run `$E g $VM running 'sleep 600'`. It must list one process.
3. Press close pane `$E press $VM close-pane`.
4. **Shot** `t03-6-dialog`. Check: a dialog titled "Close Pane?" says a
   process is still running, with "Close" and "Cancel" buttons.
5. `$E key $VM Escape`. **Shot** `t03-6-cancelled`. Check: the dialog is gone
   and both panes are still there.
6. Run `$E g $VM running 'sleep 600'`. Check: still one process.
7. Press close pane again, then `$E key $VM Return` (confirms "Close").
8. **Shot** `t03-6-confirmed`. Check: one pane remains.
9. Run `$E g $VM running 'sleep 600'`. Check: `no process matches`.

## T03.7 Several tabs

1. Press new tab `$E press $VM new-tab`, then wait `sleep 3` (other terminals already show a prompt).
2. `$E type $VM 'echo tab-two-$((4*4))'`, `$E key $VM Return`, `$E g $VM wait 'tab-two-16'`.
3. **Shot** `t03-7-two-tabs`. Check: two tabs in the tab strip; the second is
   selected and shows `tab-two-16`.
4. Press select tab 1 `$E press $VM tab-1`.
5. **Shot** `t03-7-tab-one`. Check: the first tab is selected; its pane shows
   `lower-4` (T03.6 closed the `left-6` pane).
6. `$E key $VM ctrl+Tab`. **Shot** `t03-7-next`. Check: the second tab is
   selected again (`tab-two-16` visible).

## T03.8 Rename a tab

1. **Shot** `t03-8-before`. Find the second (selected) tab in the tab strip
   and note the center of its title text.
2. Right-click there: `$E right-click $VM <x> <y>`. This uses pixels because both
   tabs can have the same title, so a name can't pick one.
3. **Shot** `t03-8-menu`. Check: a menu lists "New Tab to the Left", "New
   Tab to the Right", "Move to New Window", "Rename Tab", "Color", "Pin Tab",
   "Close Tab", "Close Other Tabs", "Close Tabs to the Left" and "Close Tabs to
   the Right" (some of the close items can be greyed out).
4. Click "Rename Tab": `$E click-element $VM 'Rename Tab'`.
5. **Shot** `t03-8-dialog`. Check: a small "Rename Tab" box with a text field
   and "Save".
6. Press select-all `$E press $VM select-all`, `$E type $VM 'E2E Renamed'`,
   `$E key $VM Return`.
7. **Shot** `t03-8-after`. Check: a tab is titled "E2E Renamed".

## T03.9 Detach and reopen a terminal

1. Make sure the tab showing `tab-two-16` is selected (press `$E key $VM ctrl+Tab`
   until it is, checking with a shot).
2. Press palette `$E press $VM palette`,
   `$E type $VM 'detach terminal'`, `$E key $VM Return`.
3. **Shot** `t03-9-detached`. Check: that tab is gone; one tab remains.
4. Run `$E g $VM muxy session list`. Check: one line ends with `Live	false`
   (a terminal still running with no app showing it).
5. Press existing terminals `$E press $VM existing-terminals`.
6. **Shot** `t03-9-list`. Check: a list with one "Terminal <number>" row.
7. `$E key $VM Return`. Then `$E g $VM wait 'tab-two-16'`.
8. **Shot** `t03-9-reopened`. Check: a new tab shows the earlier output
   `tab-two-16`.

# S12 Terminal UI client

The `muxy` terminal UI is an app of its own: it restores its layout, stays
quiet when nothing changes, survives being suspended, follows resizes, and
shares one layout between windows. These tests run it inside a desktop
terminal against the isolated `~/e2e/tui-profile`, so its server is separate
from the desktop's.

Start: run `$E g $VM reset`, then press new tab `$E press $VM new-tab` and
`$E g $VM wait 'muxy@'`. Tests continue from each other in order.

The terminal UI draws "Ctrl-B ? help" at the top right of its tab row; the
steps use that text to know it is on screen. Its keys start with Ctrl-B: press
`$E key $VM ctrl+b`, then type the next key with `$E type $VM`.

## T12.1 First run opens one shell in Home

1. `$E type $VM '$HOME/.local/bin/muxy-e2e tui-record'`, `$E key $VM Return`.
2. Run `$E g $VM wait 'Ctrl-B ? help' 30`.
3. **Shot** `t12-1-tui`. Check: the desktop pane shows the terminal UI: a
   "projects" column on its left listing `Home`, one tab in its tab row, and
   "Ctrl-B ? help" at its top right.
4. `$E type $VM 'echo tui-first-$((6*7))'`, `$E key $VM Return`,
   `$E g $VM wait 'tui-first-42'`.
5. Run `$E g $VM tui session list`. Check: exactly one line, ending in `Live	true`.
6. Run `$E g $VM muxy session list`. Check: the id from step 5 is **not** listed
   (the desktop's server doesn't own the terminal UI's terminals).

## T12.2 An idle terminal UI writes almost nothing

1. Wait: `sleep 3`. Don't press anything during the next step.
2. Run `$E g $VM tui-idle`. Check: it exits 0 and prints
   `idle bytes in 10 seconds: <n> (limit 320)` with `<n>` at most 320. Quote the line.
   If it exits non-zero, mark FAIL and quote the line.

## T12.3 Splits, focus and closing a pane

1. `$E key $VM ctrl+b`, then `$E type $VM '%'` (split side by side). Wait: `sleep 2`.
2. `$E type $VM 'echo tui-right-$((5*5))'`, `$E key $VM Return`,
   `$E g $VM wait 'tui-right-25'`.
3. **Shot** `t12-3-split`. Check: inside the terminal UI, the tab shows two panes
   side by side; `tui-first-42` on the left, `tui-right-25` on the right.
4. Run `$E g $VM tui session list`. Check: two lines.
5. `$E key $VM ctrl+b`, then `$E type $VM 'x'` (close the focused pane). Wait: `sleep 2`.
   If the terminal UI asks to confirm, take **Shot** `t12-3-confirm` and press
   `$E type $VM 'y'`.
6. Run `$E g $VM gone 'tui-right-25'`. Check: it succeeds.
7. Run `$E g $VM tui session list`. Check: one line again (closing the pane ended
   its terminal).

## T12.4 Suspend and resume

1. Run `$E g $VM tui-signal suspend`. Check: the printed state column contains `T`
   (stopped). Quote the line.
2. Run `$E g $VM gone 'Ctrl-B ? help'`. Check: it succeeds (the terminal UI left
   the screen to the shell it was started from).
3. **Shot** `t12-4-suspended`. Check: the desktop pane shows the plain shell screen
   with the `muxy-e2e tui-record` command, not the terminal UI.
4. Run `$E g $VM tui-signal resume`. Check: the state column no longer contains `T`.
5. Run `$E g $VM wait 'Ctrl-B ? help'`.
6. `$E type $VM 'echo resumed-$((8*8))'`, `$E key $VM Return`,
   `$E g $VM wait 'resumed-64'`.
7. **Shot** `t12-4-resumed`. Check: the terminal UI is drawn whole again (projects
   column, tab row, "Ctrl-B ? help") and shows `resumed-64`.

## T12.5 Detach; the layout and the terminal come back

1. `$E key $VM ctrl+b`, then `$E type $VM 'd'`. Wait: `sleep 2`.
2. Run `$E g $VM gone 'Ctrl-B ? help'`. Check: it succeeds.
3. Run `$E g $VM tui session list`. Check: one line, ending in `Live	false`
   (the terminal kept running with no app).
4. `$E type $VM 'clear'`, `$E key $VM Return`. Wait: `sleep 1`.
5. `$E type $VM '$HOME/.local/bin/muxy-e2e tui-primary'`, `$E key $VM Return`.
6. Run `$E g $VM wait 'Ctrl-B ? help' 30`, then `$E g $VM wait 'resumed-64'`.
7. **Shot** `t12-5-restored`. Check: the terminal UI is back with one tab and one
   pane that still shows `resumed-64`.

## T12.6 The terminal UI follows resizes

1. **Shot** `t12-6-wide`. Note how far right "Ctrl-B ? help" is drawn.
2. Press split right `$E press $VM split-right`. Wait: `sleep 3`
   (the desktop's other terminal already shows a prompt).
3. **Shot** `t12-6-narrow`. Check: the terminal UI now fills only the left desktop
   pane, and it is redrawn to fit: its projects column and tab row end inside the
   **left** pane, and long lines wrap at that pane's right edge instead of being
   cut off. (The "Ctrl-B ? help" hint is left out when the tab row is too narrow
   for it, so don't look for it here.)

## T12.7 Two windows share one layout

1. The right desktop pane is focused. `$E type $VM '$HOME/.local/bin/muxy-e2e tui'`,
   `$E key $VM Return`. Wait: `sleep 4`.
2. **Shot** `t12-7-second`. Check: both desktop panes show a terminal UI, and both
   show `resumed-64` (the same tab).
3. In the right one: `$E key $VM ctrl+b`, then `$E type $VM 'c'` (new tab). Wait: `sleep 2`.
4. `$E type $VM 'echo shared-$((7*11))'`, `$E key $VM Return`,
   `$E g $VM wait 'shared-77'`.
5. **Shot** `t12-7-shared`. Check: the tab row of **both** terminal UIs has two
   tabs (the left one shows the new tab too, even though it didn't make it).

## T12.8 Clean up the terminal UI server

1. `$E key $VM ctrl+b`, then `$E type $VM 'd'`. Wait: `sleep 2`.
2. Press focus left `$E press $VM focus-left`, then `$E key $VM ctrl+b`,
   `$E type $VM 'd'`. Wait: `sleep 2`.
3. Run `$E g $VM gone 'Ctrl-B ? help'`. Check: it succeeds.
4. Run `$E g $VM tui server stop --force`. Check: it exits 0.
5. Wait: `sleep 3` (the server exits a moment after the stop command returns).
6. Run `$E g $VM running 'tui-profile'`. Check: it prints
   `no process matches: tui-profile` (expected; it exits 1).

Needs verification before first use:
- T12.3 step 5: whether `Ctrl-B x` asks for confirmation at an idle shell prompt
  (docs say it asks only when a program is running), and which key confirms.
- T12.4 step 3: what the desktop pane shows while the terminal UI is stopped
  (`restore()` leaves the alternate screen, so the shell screen is expected).
- T12.6: macOS `script` doesn't pass resizes on, which is why T12.5 switches to
  `tui-primary`; confirm the redraw on both platforms.
- T12.8 step 5: `running` matches command lines; the terminal UI server may not
  carry `tui-profile` in its command line. If so, use `$E g $VM tui server status`
  (it never starts the server) and expect a not-running message instead.

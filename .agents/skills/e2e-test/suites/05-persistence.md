# S05 Persistence and lifecycle

Muxy's core promise: terminals outlive the app, the app restores its
layout, the server keeps saved output, and End All Sessions ends everything.

Start: run `$E g $VM reset`. Tests continue from each other in order.

## T05.1 Build a layout to restore

1. Press add project `$E press $VM add-project`, `$E type $VM '~/e2e/alpha'`,
   `sleep 1`, `$E key $VM Return`, then `$E g $VM wait 'muxy@'`.
2. Start a background writer in this pane:
   `$E type $VM 'while true; do date +%s >> ~/e2e/tick.txt; sleep 1; done'`, `$E key $VM Return`.
3. Press split right `$E press $VM split-right`, `sleep 2`.
4. `$E type $VM 'echo before-quit-$((9*9))'`, `$E key $VM Return`, `$E g $VM wait 'before-quit-81'`.
5. **Shot** `t05-1-layout`. Check: project A selected, one tab with two panes;
   `before-quit-81` in the right pane.

## T05.2 Quitting the app keeps terminals running

1. Press quit `$E press $VM quit`. Wait: `sleep 3`.
2. Run `$E g $VM app-pid`. Check: it prints nothing (the app has exited).
3. Run `$E g $VM muxy session list`. Check: two lines, each ending in
   `Live	false` (running, no app attached).
4. Run `$E g $VM lines '~/e2e/tick.txt'`, `sleep 4`, run it again. Check: the
   second number is larger (the loop still writes).
5. **Shot** `t05-2-no-app`. Check: no Muxy window.

## T05.3 Relaunch restores the layout and the screens

1. Run `$E g $VM launch`.
2. **Shot** `t05-3-restored`. Check: project A is selected, its tab has the
   same two panes side by side, and the right pane shows `before-quit-81`.
3. Run `$E g $VM muxy session list`. Check: the same two session ids as in
   T05.2 step 3, now ending in `Live	true`.
4. Run `$E g $VM lines '~/e2e/tick.txt'` twice, 4 seconds apart. Check: it grows.

## T05.4 Stopping the server keeps saved output

1. Focus the left pane: press focus left `$E press $VM focus-left`, then
   `$E key $VM ctrl+c` (stops the loop).
2. Run `$E g $VM muxy session list` and write down both session ids.
3. Run `$E g $VM muxy server stop --force`. Wait: `sleep 3`.
4. **Shot** `t05-4-stopped`. Check: the status bar says "Server · Disconnected"
   (red dot), and both terminal panes are still there, showing their last
   screen (they start new shells once the server is back).
5. Run `$E g $VM muxy session list --all`. Check: **both** ids from step 2 are
   listed, each with `Ended`.
6. For each id run `$E g $VM muxy session read-screen <id> --saved`. Check: the
   right pane's id shows `before-quit-81`; the left pane's shows `while true`.

## T05.5 The app reconnects when needed

1. Press new tab `$E press $VM new-tab`. Then `$E g $VM wait 'muxy@' 20`.
2. **Shot** `t05-5-reconnected`. Check: a terminal with a prompt is open and the
   status bar says "Server · Connected".

## T05.6 End All Sessions and Quit

1. Press split right `$E press $VM split-right`, `sleep 2`.
2. Run `$E g $VM muxy session list`. Check: at least two lines.
3. Press palette `$E press $VM palette`,
   `$E type $VM 'end all sessions'`, **Shot** `t05-6-palette` (check the row
   "End All Sessions and Quit" is listed), then `$E key $VM Return`.
4. Wait: `sleep 3`. If a confirmation dialog is shown, take **Shot**
   `t05-6-confirm` and press `$E key $VM Return`.
5. Run `$E g $VM app-pid`. Check: empty (the app quit).
6. Run `$E g $VM muxy session list`. Check: no lines (no live terminals).

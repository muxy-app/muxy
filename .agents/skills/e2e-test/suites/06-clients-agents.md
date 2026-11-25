# S06 Clients and agents

Other clients share the app's terminals: the CLI types into a visible
terminal, the terminal UI runs inside Muxy, and programs report status that
the app shows.

Start: run `$E g $VM reset`. Tests continue from each other in order.

## T06.1 The CLI types into a terminal the app shows

1. Press new tab `$E press $VM new-tab`, `$E g $VM wait 'muxy@'`.
2. Run `$E g $VM muxy session list` and note the id.
3. Run `$E g $VM muxy session send <id> 'echo from-cli-$((3*7))'`, then
   `$E g $VM muxy session send-keys <id> Enter`.
4. Run `$E g $VM wait 'from-cli-21'`.
5. **Shot** `t06-1-cli-input`. Check: the app's terminal shows the command and
   `from-cli-21` without you typing in the app.

## T06.2 The terminal UI runs inside a Muxy terminal

1. `$E type $VM 'muxy'`, `$E key $VM Return`. Wait: `sleep 3`.
2. **Shot** `t06-2-tui`. Check: the pane now shows the terminal UI: a
   "projects" column on its left listing `Home`, a tab row at its top, and
   "Ctrl-B ? help" at its top right.
3. `$E key $VM ctrl+b`, then `$E type $VM 'c'` (new tab in the terminal UI). Wait: `sleep 2`.
4. `$E type $VM 'echo tui-$((2*21))'`, `$E key $VM Return`, `$E g $VM wait 'tui-42'`.
5. **Shot** `t06-2-tui-tab`. Check: the terminal UI's tab row has two tabs,
   and the new one shows `tui-42`.

## T06.3 Detach the terminal UI; its terminals keep running

1. `$E key $VM ctrl+b`, then `$E type $VM 'd'`. Wait: `sleep 2`.
2. **Shot** `t06-3-detached`. Check: the pane shows a normal shell prompt again.
3. Run `$E g $VM muxy session list`. Check: at least three lines, and every
   line has `Live	true` before its folder (the app shows the terminal UI's
   terminals as tabs, so they stay live after it detaches).

## T06.4 The app shows the terminal UI's terminals as tabs

1. Run `$E g $VM find 'tui-42'`. It prints one line `<id>: tui-42`; note the id
   (if it prints nothing, mark FAIL: the terminal UI's terminal is gone).
2. **Shot** `t06-4-tabs`. Check: the tab strip has three tabs.
3. Press existing terminals `$E press $VM existing-terminals`,
   `$E type $VM '<id>'` (the digits from step 1).
4. **Shot** `t06-4-list`. Check: the list says "No matching terminals" (the
   app already shows that terminal). `$E key $VM Escape`.
5. Press next tab `$E press $VM next-tab`. **Shot** `t06-4-next-1`.
6. Press next tab `$E press $VM next-tab`. **Shot** `t06-4-next-2`.
7. Check: one of the shots from steps 5 and 6 shows a tab with `tui-42`, the
   output typed in the terminal UI.

## T06.5 Program status on a tab

1. Press new tab `$E press $VM new-tab`, then wait `sleep 3` (other terminals already show a prompt).
2. `$E type $VM 'muxy status error --app deploy --msg Failed'`, `$E key $VM Return`.
   Wait: `sleep 2`.
3. **Shot** `t06-5-error`. Check: this tab's title in the tab strip has a red
   status icon (a small red or magenta ring) left of the title.
4. `$E type $VM 'muxy status clear'`, `$E key $VM Return`. Wait: `sleep 2`.
5. **Shot** `t06-5-cleared`. Check: the red icon is gone (a terminal icon is back).

## T06.6 An agent waiting for input

1. Press new tab `$E press $VM new-tab`, then wait `sleep 3` (other terminals already show a prompt).
2. Type exactly:
   `$E type $VM "sh -c 'muxy status blocked --app claude --kind question --msg Approve; sleep 120'"`
   then `$E key $VM Return`. Wait: `sleep 3`.
3. Run `$E g $VM muxy activity list`. Check: one line containing `Claude` and `blocked`.
4. Press another tab so this one is in the background: select tab 1
   `$E press $VM tab-1`.
5. Press agents panel `$E press $VM agents-panel`.
6. **Shot** `t06-6-agents`. Check: an "Agents" panel says "1 need input" and
   lists one row; the background tab has an agent icon with a colored dot.
7. `$E key $VM Escape` closes the panel.

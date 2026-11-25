# S10 Composer

The composer writes a draft and sends it to the active terminal, or to every
visible pane.

Start: run `$E g $VM reset`, then press new tab `$E press $VM new-tab`
and `$E g $VM wait 'muxy@'`. Tests continue from each other in order.

## T10.1 Send a draft to the terminal

1. Press composer `$E press $VM composer`. Wait: `sleep 1`.
2. `$E type $VM 'echo composed-$((6*6))'`.
3. **Shot** `t10-1-draft`. Check: a "Rich Input" panel on the right holds the
   text `echo composed-$((6*6))`; the terminal on the left doesn't show it yet.
4. Press send `$E press $VM composer-send`.
5. Run `$E g $VM wait 'composed-36'`.
6. **Shot** `t10-1-sent`. Check: the terminal shows the command and `composed-36`.

## T10.2 Send to all split panes

1. Press `$E key $VM Escape` to leave the composer, then split right
   `$E press $VM split-right`, `sleep 2`.
2. Press composer `$E press $VM composer`. Wait: `sleep 1`.
3. **Shot** `t10-2-composer`. Click the `…` button: `$E click-element $VM 'More Composer Actions'`.
4. **Shot** `t10-2-menu`. Check: a menu with "Send to All Split Panes". Pick it. On Linux:
   `$E click-element $VM 'Send to All Split Panes'`. On macOS: `$E key $VM Down Return`,
   because this menu is a native one outside Muxy's window, so its items have no ref.
5. Press select-all `$E press $VM select-all` in the composer, then
   `$E type $VM 'echo everywhere-$((7*7))'`, then press send
   `$E press $VM composer-send`.
6. Run `$E g $VM find 'everywhere-49'`. Check: two lines with two different ids.
7. **Shot** `t10-2-sent`. Check: both panes show `everywhere-49`.

# S02 Terminal

Typing, output, rendering, interrupting, history, find, clipboard, shell
integration and font size in one terminal.

Start: run `$E g $VM reset`. Tests T02.1–T02.9 continue from each other in
order; don't reset between them.

## T02.1 Open a terminal and run a command

1. Press new tab `$E press $VM new-tab`.
2. Run `$E g $VM wait 'muxy@'`. It must print one line.
3. `$E type $VM 'echo done-$((40+2))'` then `$E key $VM Return`.
4. Run `$E g $VM wait 'done-42'`. It must print a line ending in `done-42`.
5. **Shot** `t02-1-output`. Check: one tab in the tab strip; the terminal shows
   the command line and, under it, `done-42`, then a new prompt.

## T02.2 Unicode, wide characters and colors

1. Type this and press `Return`:
   `$E type $VM 'python3 -c "print(chr(0x2713), chr(0xfc), chr(0x4e2d)+chr(0x6587), chr(0x1F600)); print(chr(27)+chr(91)+chr(51)+chr(49)+chr(109)+chr(82)+chr(69)+chr(68)+chr(27)+chr(91)+chr(48)+chr(109))"'`
2. Run `$E g $VM wait 'RED'`.
3. **Shot** `t02-2-unicode`. Check, on the line under the command:
   - a check mark `✓`, `ü`, two Chinese characters `中文`, and a smiley emoji;
     none shown as empty boxes or question marks;
   - on the next line, the word `RED` drawn in red.

## T02.3 Ctrl+C stops a running program

1. `$E type $VM 'sleep 300'`, `$E key $VM Return`.
2. Run `$E g $VM running 'sleep 300'`. It must list one process.
3. `$E key $VM ctrl+c`.
4. `$E type $VM 'echo after-$((1+1))'`, `$E key $VM Return`, then
   `$E g $VM wait 'after-2'`.
5. Run `$E g $VM running 'sleep 300'`. Check: it prints `no process matches`.

## T02.4 Scrollback

1. `$E type $VM 'seq 1 3000'`, `$E key $VM Return`, `$E g $VM wait '3000'`.
2. Press scroll page up `$E press $VM scroll-page-up` twice.
3. **Shot** `t02-4-scrolled`. Check: the terminal shows numbers smaller than
   2950 and does not show `3000`.
4. Run `$E g $VM muxy session list` and copy the first column (the session id).
   Run `$E g $VM muxy session search <id> 1500`. Check: the output is JSON whose
   `"matches"` list has at least one entry (`"matches":[{"row":...`), and
   `"total_rows"` is above 2900 (the server holds the history).

## T02.5 Find

1. Press find `$E press $VM find`.
2. `$E type $VM '1234'`, then `$E key $VM Return`.
3. **Shot** `t02-5-find`. Check: a find bar is at the top of the terminal with
   `1234` in it and a count such as `1 of 1`; the line `1234` is visible and
   highlighted.
4. `$E key $VM Escape`. **Shot** `t02-5-closed`. Check: the find bar is gone.

## T02.6 Paste

1. `$E key $VM ctrl+c` (clears the prompt line).
2. Run `$E g $VM clip-set 'echo pasted-$((5*5))'`.
3. Press paste `$E press $VM paste`, then `$E key $VM Return`.
4. Run `$E g $VM wait 'pasted-25'`. It must print a line.

## T02.7 Shell integration, select command output, copy

1. Run `$E g $VM muxy settings set default-shell /bin/zsh`. It prints `ok`.
2. Press new tab `$E press $VM new-tab`. Wait: `sleep 3`. (This
   tab runs zsh; on Linux its prompt ends in `%`.)
3. `$E type $VM 'echo first-$((1+1))'`, `$E key $VM Return`, `$E g $VM wait 'first-2'`.
4. `$E type $VM 'seq 101 160'`, `$E key $VM Return`, `$E g $VM wait '160'`.
5. Press palette `$E press $VM palette`,
   `$E type $VM 'select command output'`, **Shot** `t02-7-palette`. Check: the
   list shows "Select Command Output". Press `$E key $VM Return`.
6. **Shot** `t02-7-selected`. Check: the numbers 101…160 are highlighted as a
   selection and the prompt lines are not.
7. Press copy `$E press $VM copy`.
8. Run `$E g $VM clip-get`. Check: the output starts with the lines `101`,
   `102`, `103` and contains no `seq` or `echo`.

## T02.8 Jump to an earlier prompt

1. `$E key $VM ctrl+c` (clears the selection and the prompt line; don't use
   Escape here, zsh would swallow the next letter).
2. `$E type $VM 'seq 201 260'`, `$E key $VM Return`, `$E g $VM wait '260'`.
3. Press previous prompt `$E press $VM previous-prompt` three times (the
   empty prompt left by step 1's Ctrl+C counts as one).
4. **Shot** `t02-8-prompt`. Check: the view has scrolled up: the command line
   `seq 101 160` is visible and `260` is not.

## T02.9 Font size

1. **Shot** `t02-9-before`.
2. Press bigger font `$E press $VM font-bigger` three times.
3. **Shot** `t02-9-bigger`. Check: the terminal text is clearly larger than in
   `t02-9-before` (fewer characters fit on a line).
4. Press smaller font `$E press $VM font-smaller` three times.
5. **Shot** `t02-9-after`. Check: the text is back to the size in `t02-9-before`.

End of suite: run `$E g $VM muxy settings set default-shell default`.

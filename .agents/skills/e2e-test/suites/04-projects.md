# S04 Projects

Adding projects in the app, Git status, terminals made by the CLI appearing
in the app, and renames and deletes made outside the app.

Start: run `$E g $VM reset`. The fixtures are `~/e2e/alpha` (a Git repository)
and `~/e2e/beta` (a plain folder). Tests continue from each other in order.

## T04.1 Add a project from the app

1. Press add project `$E press $VM add-project`.
2. `$E type $VM '~/e2e/alpha'`. Wait: `sleep 1`.
3. **Shot** `t04-1-picker`. Check: a picker shows `~/e2e/alpha/` in its field,
   an `alpha` row, and an "Add Project" button.
4. `$E key $VM Return`. Then `$E g $VM wait 'muxy@'`.
5. `$E type $VM 'echo "dir=$PWD"'`, `$E key $VM Return`, `$E g $VM wait 'dir=/'`.
   Check: the printed line ends with `/e2e/alpha`.
6. **Shot** `t04-1-added`. Check: the sidebar has a second icon under Home
   (letter A, selected); a terminal tab is open; the status bar shows a
   branch `main` and `Clean`.
7. Run `$E g $VM muxy project list`. Check: a line named `alpha` whose folder
   ends with `/e2e/alpha`.

## T04.2 Git changes show in the status bar

1. `$E type $VM 'echo change >> README.md'`, `$E key $VM Return`. Wait: `sleep 5`.
2. **Shot** `t04-2-changed`. Check: the status bar no longer says `Clean`; it
   shows a change count (for example `1`) or changed-line numbers.
3. `$E type $VM 'git checkout -- README.md'`, `$E key $VM Return`. Wait: `sleep 5`.
4. **Shot** `t04-2-clean`. Check: the status bar says `Clean` again.

## T04.3 A terminal made by the CLI appears in the app

1. Run `$E g $VM muxy session create alpha -- 'echo cli-made-$((8*8))'`. It
   prints a session id; note it.
2. Run `$E g $VM wait 'cli-made-64'`.
3. Press existing terminals `$E press $VM existing-terminals`.
4. **Shot** `t04-3-list`. Check: a row "Terminal <the id>" with the alpha
   folder and "No owner".
5. `$E key $VM Return`. **Shot** `t04-3-open`. Check: a new tab shows `cli-made-64`.

## T04.4 `muxy session show` focuses the terminal

1. Run `$E g $VM muxy session create alpha -- 'echo shown-$((9*9))'` and note the id.
2. Run `$E g $VM muxy session show <id>`.
3. Wait: `sleep 2`. **Shot** `t04-4-shown`. Check: the selected tab shows `shown-81`.

## T04.5 A rename made by the CLI reaches the app

1. Run `$E g $VM muxy project rename alpha Zulu`.
2. Press sidebar toggle `$E press $VM sidebar` to expand the sidebar.
3. **Shot** `t04-5-renamed`. Check: the sidebar lists `Home` and `Zulu`; no
   `alpha` (worktree rows under it, if any, don't count).

## T04.6 Switch projects

1. Run `$E g $VM muxy project add '~/e2e/beta'` (keep the quotes).
2. Wait: `sleep 2`. **Shot** `t04-6-three`. Check: the sidebar lists `Home`,
   `Zulu` and `beta`.
3. Press next project `$E press $VM next-project`, then take a shot
   (`t04-6-next-1`). If `beta` isn't selected, press it again and take
   `t04-6-next-2`; stop as soon as `beta` is selected, at most 3 presses.
4. Check, on the shot where `beta` is selected: it says "No tabs in beta".
5. Press previous project `$E press $VM previous-project` once. **Shot** `t04-6-prev`. Check: the
   selected project changed and shows its own tabs.

## T04.7 Delete a project from the CLI

1. Run `$E g $VM muxy session list`; count lines whose folder ends in `/e2e/alpha`.
2. Run `$E g $VM muxy project delete Zulu --yes`.
3. Wait: `sleep 2`. **Shot** `t04-7-deleted`. Check: `Zulu` is gone from the
   sidebar; `Home` and `beta` remain.
4. Run `$E g $VM muxy session list`. Check: no line's folder ends in `/e2e/alpha`.
5. Run `$E g $VM lines '~/e2e/alpha/README.md'`. Check: it prints `2`
   (the files stay on disk).

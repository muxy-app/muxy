# S11 Worktrees end to end

Everything a Git worktree goes through: found, created from the app and the
CLI, worked in, switched to, renamed, removed, lost, and hidden. S07 has the
short version; this suite goes deep.

Start: run `$E g $VM reset`. The `gamma` fixture is a Git repository with a
`topic` branch and `.muxy/worktree.json`, whose setup and teardown commands
append `setup-<branch>` and `teardown-<branch>` to `~/e2e/hook-log.txt`.
Tests continue from each other in order.

`muxy worktree list gamma` prints `<project id> <branch> <folder>`. A line that
starts with its branch has no project id: Muxy doesn't show that worktree.

Commands marked "refused" must print a `muxy:` error and exit non-zero; that is
the expected result, so quote the message and keep going.

Muxy's macOS confirmation dialogs are native: when a step says **Dialog**, also
save `$E desktop $VM look --app muxy-app` as evidence. On Linux the shot alone
is enough.

## T11.1 Where worktrees are refused

1. Run `$E g $VM muxy project add '~/e2e/beta'`.
2. Run `$E g $VM muxy worktree create beta nope`. Check: refused.
3. Run `$E g $VM muxy worktree create Home nope`. Check: refused.
4. Run `$E g $VM exists '~/.muxy/worktrees/beta/nope'`. Check: `no`.
5. Wait: `sleep 2`. **Shot** `t11-1-sidebar`. Click the `B` tile in the sidebar:
   `$E click-element $VM beta --role item`.
6. Press palette `$E press $VM palette`, `$E type $VM 'new worktree'`.
7. **Shot** `t11-1-palette`. Check: "New Worktree…" is listed greyed out.
   `$E key $VM Escape`.
8. Run `$E g $VM muxy project delete beta --yes`.

## T11.2 A repository that has a worktree shows it when added

1. Run `$E g $VM git '~/e2e/gamma' worktree add -b pre-made '~/.muxy/worktrees/outside/pre-made'`.
2. Press add project `$E press $VM add-project`, `$E type $VM '~/e2e/gamma'`,
   `sleep 1`, `$E key $VM Return`, then `$E g $VM wait 'muxy@'`. Wait: `sleep 3`.
3. Run `$E g $VM muxy worktree list gamma`. Check: the `pre-made` line starts
   with a project id.
4. Press sidebar toggle `$E press $VM sidebar`.
5. **Shot** `t11-2-sidebar`. Check: the `gamma` row ends with a small `>` chevron.
   Click it: `$E click-element $VM 'Expand Worktrees'`.
6. **Shot** `t11-2-expanded`. Check: under `gamma`, rows `gamma` (marked PRIMARY),
   `pre-made`, and "New Worktree".

## T11.3 Made and deleted outside Muxy

1. Run `$E g $VM git '~/e2e/gamma' worktree add -b from-git '~/.muxy/worktrees/outside/from-git'`.
2. Wait: `sleep 5`. **Shot** `t11-3-found`. Check: a `from-git` row is in the
   sidebar.
3. Run `$E g $VM git '~/e2e/gamma' worktree remove '~/.muxy/worktrees/outside/from-git'`.
4. Wait: `sleep 5`. **Shot** `t11-3-gone`. Check: the `from-git` row is gone.
5. Run `$E g $VM muxy project list`. Check: no `from-git` line.

## T11.4 The New Worktree form, with setup commands

1. Click "New Worktree": `$E click-element $VM 'New Worktree'`. **Shot** `t11-4-form`. Check: a "New Worktree"
   dialog with Name, "Create new branch" selected, Branch name, Base branch
   `main`, Location with `Default` selected, and Setup commands showing
   `[Project] echo setup-$MUXY_WORKTREE_BRANCH >> ~/e2e/hook-log.txt` with its
   toggle off.
2. `$E type $VM 'ui-one'`.
3. Click the Branch name field: `$E click-element $VM 'Branch name' --role textfield`. Press
   `$E press $VM select-all`, then
   `$E type $VM 'ui-branch'`.
4. Click the toggle beside "Run these commands after creating the worktree":
   `$E click-element $VM 'Run these commands after creating the worktree'`.
5. **Shot** `t11-4-filled`. Check: Name `ui-one`, Branch name `ui-branch`, the
   toggle on, and a location line ending in `/.muxy/worktrees/gamma/ui-one`.
6. Click **Create**: `$E click-element $VM Create --role button`. (Return only
   submits while a text field has focus.) Wait:
   `sleep 4`.
7. **Shot** `t11-4-created`. Check: a `ui-one` row, selected; the main area says
   "No tabs in ui-one"; the status bar shows branch `ui-branch`.
8. Run `$E g $VM git '~/.muxy/worktrees/gamma/ui-one' branch --show-current`.
   Check: `ui-branch`.
9. Run `$E g $VM file '~/e2e/hook-log.txt'`. Check: exactly one line,
   `setup-ui-branch`.

## T11.5 A worktree has its own tabs and terminals

1. Press new tab `$E press $VM new-tab`, wait `sleep 3`,
   `$E type $VM 'echo "wt=$(git branch --show-current) dir=$PWD"'`,
   `$E key $VM Return`, `$E g $VM wait 'wt=ui-branch'`.
   Check: the line ends in `/.muxy/worktrees/gamma/ui-one`.
2. Click the `gamma` row marked PRIMARY: `$E click-element $VM gamma --role item --nth 2`
   (the project row and its PRIMARY worktree row are both named `gamma`; the worktree row comes second). **Shot** `t11-5-primary`. Check: the tab
   strip has only the `gamma` tab and the status bar shows branch `main`.
3. Click the `ui-one` row: `$E click-element $VM ui-one --role item`. **Shot** `t11-5-worktree`. Check: its tab shows the
   `wt=ui-branch` line and the status bar shows branch `ui-branch`.

## T11.6 Creating from the CLI

1. Run `$E g $VM muxy worktree create gamma wt-base --branch wt-base-branch --base topic`.
2. Run `$E g $VM git '~/.muxy/worktrees/gamma/wt-base' branch --show-current`.
   Check: `wt-base-branch`.
3. Run `$E g $VM lines '~/.muxy/worktrees/gamma/wt-base/topic.txt'`. Check: `1`
   (it started from `topic`).
4. Run `$E g $VM muxy worktree create gamma wt-new --branch topic`. Check:
   refused (that branch exists).
5. Run `$E g $VM muxy worktree create gamma wt-existing --branch topic --existing --hooks`.
   Check: it prints a `setup:` line and the new project.
6. Run `$E g $VM file '~/e2e/hook-log.txt'`. Check: the last line is `setup-topic`.
7. Run `$E g $VM muxy worktree create gamma wt-again --branch topic --existing`.
   Check: refused (`topic` is checked out in `wt-existing`).
8. Run `$E g $VM exists '~/.muxy/worktrees/gamma/wt-again'`. Check: `no`.
9. Run `$E g $VM muxy worktree create gamma wt-taken --directory '~/e2e/beta'`.
   Check: refused (the folder exists).
10. Run `$E g $VM git '~/e2e/gamma' branch --list wt-taken`. Check: prints nothing.
11. Run `$E g $VM muxy worktree create gamma wt-dir --directory '~/e2e/custom-wt'`.
    Check: its line ends in `/e2e/custom-wt`.
12. Wait: `sleep 2`. **Shot** `t11-6-sidebar`. Check: rows `wt-base`, `wt-existing`
    and `wt-dir` in the sidebar.

## T11.7 Registering a worktree that exists

`alpha` is added by the CLI and never selected, so the app doesn't find the
worktree first.

1. Run `$E g $VM muxy project add '~/e2e/alpha'`.
2. Run `$E g $VM git '~/e2e/alpha' worktree add -b reg-branch '~/e2e/reg-wt'`.
3. Run `$E g $VM muxy worktree register alpha '~/e2e/reg-wt'`. Check: it prints
   a project named `reg-branch`.
4. Run the same command again. Check: refused (already registered).
5. Run `$E g $VM muxy worktree register alpha '~/e2e/beta'`. Check: refused (not
   a registered Git worktree).
6. Run `$E g $VM muxy project delete alpha --yes`.

## T11.8 Existing branch and a location template

First free the `topic` branch.

1. Run `$E g $VM muxy worktree remove wt-existing --yes`.
2. Press palette `$E press $VM palette`, `$E type $VM 'new worktree'`,
   `$E key $VM Return`. Then `$E type $VM 'ui-topic'`.
3. Click "Use existing branch":
   `$E click-element $VM 'Use existing branch'`. **Shot**
   `t11-8-existing`. Check: a Branch field replaces Branch name and Base branch. Click
   it: `$E click-element $VM Branch --nth 2`
   (the "Branch" choice group comes first, then this Branch picker).
   **Shot** `t11-8-branches`. Check: a list of branches that includes `topic`.
   `$E type $VM 'topic'`, `sleep 1`, `$E key $VM Return`.
4. **Shot** `t11-8-chosen`. Check: the Branch field shows `topic`. Click
   "Template" under Location: `$E click-element $VM Template`.
5. Click the empty template field: `$E click-element $VM Template --role textfield`.
   `$E type $VM '../fixed'`.
6. **Shot** `t11-8-bad`. Check: a red line says the template must include
   `{branch}`, and Create is greyed out.
7. `$E press $VM select-all`, then `$E type $VM '../{base-dir}.{branch}'`.
8. **Shot** `t11-8-good`. Check: the location line ends in `/e2e/gamma.topic`,
   and Create is enabled. The setup toggle is off.
9. Click **Create**: `$E click-element $VM Create --role button`. Wait: `sleep 4`.
10. Run `$E g $VM git '~/e2e/gamma.topic' branch --show-current`. Check: `topic`.
11. Run `$E g $VM file '~/e2e/hook-log.txt'`. Check: exactly two lines,
    `setup-ui-branch` and `setup-topic` (nothing ran for this worktree, or when
    `wt-existing` was removed without `--hooks`).
12. Run `$E g $VM muxy worktree create gamma cli-after`. Check: its line ends in
    `/e2e/gamma.cli-after` (the CLI uses the location the project remembers).
13. Run `$E g $VM muxy worktree remove cli-after --yes`.

## T11.9 Switch Worktree

1. Press palette `$E press $VM palette`, `$E type $VM 'switch worktree'`,
   `$E key $VM Return`.
2. **Shot** `t11-9-switcher`. Check: a "Search worktrees…" list with the current
   worktree marked by a check, `gamma` marked PRIMARY, `pre-made`, and a
   "New Worktree…" button.
3. `$E type $VM 'pre'`. **Shot** `t11-9-filtered`. Check: only `pre-made` is listed.
4. `$E key $VM Return`. **Shot** `t11-9-switched`. Check: "No tabs in pre-made",
   and the status bar shows branch `pre-made`.

## T11.10 Rename from the sidebar

1. Right-click the `ui-one` row: `$E click-element $VM ui-one --right`.
2. **Shot** `t11-10-menu`. Check: "New Terminal Tab", "Existing Terminals…" (it can
   be drawn cut off as "Existing Terminal…" beside its shortcut, which is a known
   issue, not a failure), "Apply Layout…", "Rename Worktree…", "Copy Path", and an
   item starting "Remove Worktree and File". Other items may also be listed.
3. Click "Rename Worktree…": `$E click-element $VM 'Rename Worktree…'`. `$E press $VM select-all`, `$E type $VM 'ui-renamed'`,
   `$E key $VM Return`. Wait: `sleep 2`.
4. **Shot** `t11-10-renamed`. Check: the row now reads `ui-renamed`.
5. Run `$E g $VM muxy project list`. Check: `ui-renamed` with a folder still
   ending in `/gamma/ui-one`.
6. Run `$E g $VM git '~/.muxy/worktrees/gamma/ui-one' branch --show-current`.
   Check: still `ui-branch`.

## T11.11 Uncommitted changes protect a worktree

1. Click the `ui-renamed` row: `$E click-element $VM ui-renamed --role item`. Wait: `sleep 2`.
   `$E type $VM 'echo wip > dirty.txt; echo made-$((40+2))'`, `$E key $VM Return`,
   `$E g $VM wait 'made-42'`. Wait: `sleep 3`.
2. Run `$E g $VM muxy worktree remove ui-renamed`. Check: refused (asks for `--yes`).
3. Run `$E g $VM muxy worktree remove ui-renamed --yes`. Check: refused (says it
   has uncommitted changes and names `--force`).
4. Run `$E g $VM exists '~/.muxy/worktrees/gamma/ui-one/dirty.txt'`. Check: `yes`.
5. Open the row's menu: `$E click-element $VM ui-renamed --right`. Click
   `$E click-element $VM 'Remove Worktree and Files…'`.
6. **Shot** `t11-11-dialog` and **Dialog**. Check: "Confirm Git Operation", a message
   saying it will permanently discard uncommitted changes of `ui-renamed`, the
   `[Project] echo teardown-...` command, an unticked "Run the teardown commands
   shown above", and "Cancel" and "Confirm".
7. Click "Cancel": `$E click-element $VM Cancel`. Wait: `sleep 1`.
8. Run `$E g $VM exists '~/.muxy/worktrees/gamma/ui-one/dirty.txt'`. Check: `yes`.

## T11.12 Removing from the app moves you to the primary

Continues with `ui-renamed` selected and dirty.

1. Open the row's menu: `$E click-element $VM ui-renamed --right`. Click
   `$E click-element $VM 'Remove Worktree and Files…'`.
2. Tick "Run the teardown commands shown above":
   `$E click-element $VM 'Run the teardown commands shown above'`. **Shot**
   `t11-12-ticked`. Check: it is ticked.
3. Click "Confirm": `$E click-element $VM Confirm`. Wait: `sleep 4`.
4. **Shot** `t11-12-removed`. Check: the `ui-renamed` row is gone, the PRIMARY `gamma`
   row is selected, its tab shows, and the status bar shows branch `main`.
5. Run `$E g $VM exists '~/.muxy/worktrees/gamma/ui-one'`. Check: `no`.
6. Run `$E g $VM file '~/e2e/hook-log.txt'`. Check: the last line is
   `teardown-ui-branch`.
7. Run `$E g $VM git '~/e2e/gamma' branch --list ui-branch`. Check: it prints
   `ui-branch` (removing a worktree keeps its branch).
8. Run `$E g $VM muxy session list`. Check: no line's folder ends in `/gamma/ui-one`.

## T11.13 Removing from the CLI

1. Run `$E g $VM git '~/.muxy/worktrees/gamma/wt-base' status --short`. Check:
   prints nothing.
2. Run `$E g $VM muxy worktree remove wt-base --yes`.
3. Run `$E g $VM exists '~/.muxy/worktrees/gamma/wt-base'`. Check: `no`.
4. Wait: `sleep 2`. **Shot** `t11-13-removed`. Check: the `wt-base` row is gone.

## T11.14 Worktree terminals survive quitting the app

1. Click the `wt-dir` row: `$E click-element $VM wt-dir --role item`. Wait: `sleep 2`. Press new tab
   `$E press $VM new-tab`, wait `sleep 3`, `$E type $VM 'echo in-dir-$((40+2))'`,
   `$E key $VM Return`, `$E g $VM wait 'in-dir-42'`.
2. Run `$E g $VM quit`, then `$E g $VM muxy session list`. Check: the line whose
   folder ends in `/e2e/custom-wt` is still `Live`.
3. Run `$E g $VM launch`. Wait: `sleep 3`.
4. **Shot** `t11-14-restored`. Check: `custom-wt`'s tab shows `in-dir-42`. The
   sidebar may have collapsed `gamma`; that's fine.

## T11.15 A worktree whose folder disappears

The app checks folders when its window becomes active, so this restarts it.

1. Run `$E g $VM rm-tree '~/e2e/custom-wt'`.
2. Run `$E g $VM quit`, then `$E g $VM launch`. Wait: `sleep 3`.
3. **Shot** `t11-15-missing`. Check: "Project folder is missing".
4. If the `gamma` row shows `>`, click it: `$E click-element $VM 'Expand Worktrees'`.
   Open the `wt-dir` row's menu: `$E click-element $VM wt-dir --right`.
   If it fails with "has no place on screen" (Wayland can't place the dimmed row of a
   missing folder), take **Shot** `t11-15-row`, find the dimmed `wt-dir` row, and
   right-click the middle of its name by pixels instead: `$E right-click $VM <x> <y>`.
5. **Shot** `t11-15-menu`. Check: the only item is "Remove Worktree…". Click it:
   `$E click-element $VM 'Remove Worktree…'`.
6. **Shot** `t11-15-dialog` and **Dialog**. Check: "Remove Worktree?", a message saying its
   folder is already gone, and "Cancel" and "Remove".
7. Click "Remove": `$E click-element $VM Remove`. Wait: `sleep 3`.
8. **Shot** `t11-15-removed`. Check: the `wt-dir` row is gone and the PRIMARY
   `gamma` row is selected.
9. Run `$E g $VM muxy session list`. Check: no line's folder ends in `/e2e/custom-wt`.

## T11.16 Hiding worktrees

1. Click the `pre-made` row: `$E click-element $VM pre-made --role item`. Wait: `sleep 1`.
2. Right-click the `gamma` row at the top of its group (with the letter tile):
   `$E click-element $VM gamma --right --role item --nth 1`
   (the project row comes before its PRIMARY worktree row in the tree).
   If it fails with "has no place on screen" (Wayland can't place a selected row),
   take **Shot** `t11-16-row`, find the `gamma` row with the letter tile, and right-click
   the middle of its name by pixels instead: `$E right-click $VM <x> <y>`.
   Open "Worktrees":
   `$E click-element $VM Worktrees`. **Shot** `t11-16-submenu`. Check: "Show Worktrees" is
   ticked, with "New Worktree…" below it. Click `$E click-element $VM 'Show Worktrees'`.
3. Wait: `sleep 2`. **Shot** `t11-16-hidden`. Check: no worktree rows and no chevron;
   `gamma` is selected and shows branch `main` (you left `pre-made`).
4. Press palette `$E press $VM palette`, `$E type $VM 'new worktree'`.
   **Shot** `t11-16-palette`. Check: "New Worktree…" is greyed out. `$E key $VM Escape`.
5. Run `$E g $VM muxy worktree list gamma`. Check: `pre-made` is still listed
   with its project id (hiding doesn't remove).
6. Open the `gamma` row's menu: `$E click-element $VM gamma --right --role item`.
   If it fails with "has no place on screen" (Wayland can't place a selected row),
   take **Shot** `t11-16-row-2`, find the `gamma` row with the letter tile, and right-click
   the middle of its name by pixels instead: `$E right-click $VM <x> <y>`.
   Then
   `$E click-element $VM Worktrees`, `$E click-element $VM 'Show Worktrees'`. Wait: `sleep 2`.
7. **Shot** `t11-16-shown`. Check: the `>` chevron is back. Click it
   (`$E click-element $VM 'Expand Worktrees'`) and take
   **Shot** `t11-16-expanded`. Check: `pre-made` is listed again.

## T11.17 Removing the parent keeps every folder

1. Run `$E g $VM muxy session create pre-made -- 'echo in-child-$((40+2))'`, then
   `$E g $VM wait 'in-child-42'`.
2. Open the `gamma` row's menu: `$E click-element $VM gamma --right --role item --nth 1`.
   If it fails with "has no place on screen" (Wayland can't place a selected row),
   take **Shot** `t11-17-row`, find the `gamma` row with the letter tile, and right-click
   the middle of its name by pixels instead: `$E right-click $VM <x> <y>`.
   Click
   `$E click-element $VM 'Remove Project…'`.
3. **Shot** `t11-17-dialog` and **Dialog**. Check: "Remove Project?", naming `gamma` and
   saying its folder and files stay on disk, with "Cancel" and "Remove".
4. Click "Remove": `$E click-element $VM Remove`. Wait: `sleep 3`.
5. **Shot** `t11-17-gone`. Check: the sidebar lists only `Home`.
6. Run `$E g $VM muxy project list`. Check: only `Home`.
7. Run `$E g $VM muxy session list`. Check: nothing is listed for `gamma` or
   `pre-made`.
8. Run `$E g $VM exists '~/e2e/gamma/README.md'` and
   `$E g $VM exists '~/.muxy/worktrees/outside/pre-made/README.md'`. Check: `yes` twice.

## T11.18 Adding the parent back finds its worktrees

1. Press add project `$E press $VM add-project`, `$E type $VM '~/e2e/gamma/'`
   (with the final slash: `~/e2e/gamma.topic` also exists), `sleep 1`,
   `$E key $VM Return`, then `$E g $VM wait 'muxy@'`. Wait: `sleep 3`.
2. Run `$E g $VM muxy worktree list gamma`. Check: the `pre-made` and `topic`
   lines each start with a project id.
3. **Shot** `t11-18-sidebar`. Check: the `gamma` row ends with a `>` chevron.

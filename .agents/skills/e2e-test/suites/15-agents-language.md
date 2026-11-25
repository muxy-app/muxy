# S15 AI tools, extension agents, and language packs

Muxy hands work to AI command-line tools: the Git commit flow asks before it
runs one, Create Extension starts a coding agent in its workspace with the
`muxy` command available, and asks to install that command when it is
missing. Language packs from extensions translate the interface.

No real AI tool runs here: the fixtures install stubs. `dialog-fixture`'s
stubs exit at once; `agent-fixture`'s print `AGENT-...` lines and wait.

Start: run `$E g $VM reset`, then
`$E g $VM muxy project add '~/e2e/alpha'`. Tests continue from each other in order.

Muxy's macOS dialogs are native: when a step says **Dialog**, also save
`$E desktop $VM look --app muxy-app` as evidence. On Linux the shot alone is
enough.

## T15.1 The commit confirmation can be cancelled

1. Run `$E g $VM dialog-fixture`. Check: it prints `dialog-only AI provider stubs installed`.
2. Run `$E g $VM quit`, then `$E g $VM launch`.
3. Click alpha: `$E click-element $VM alpha --role item`. Wait: `sleep 3`.
4. **Shot** `t15-1-status`. Check: the status bar shows branch `main`, "1 Change",
   and a "Commit" item.
5. Click it: `$E click-element $VM Commit --role button`. Wait: `sleep 2`.
6. **Shot** `t15-1-dialog` (**Dialog**). Check: a dialog titled
   `Commit to "main"?` with the text "Muxy will ask the AI provider for a commit
   message, then commit.", an "AI Provider" choice showing "Claude Code",
   switches "Include unstaged changes" and "Push after committing" (drawn as
   toggles on Linux, checkboxes on macOS), and "Commit" and "Cancel" buttons.
7. Click Cancel: `$E click-element $VM Cancel`. Wait: `sleep 2`.
8. **Shot** `t15-1-cancelled`. Check: the dialog is gone; the status bar still
   says "1 Change".
9. Run `$E g $VM git '~/e2e/alpha' status --short`. Check: ` M README.md`.
10. Run `$E g $VM git '~/e2e/alpha' rev-list --count HEAD`. Check: `1` (nothing
    was committed).

## T15.2 Create Extension starts an agent in its workspace

1. Run `$E g $VM agent-fixture-remove`, then `$E g $VM agent-fixture`.
   Check: it prints `stub agents installed: claude, codex, kiro-cli`.
2. Run `$E g $VM quit`, then `$E g $VM launch`.
3. Press palette `$E press $VM palette`, `$E type $VM 'create extension'`.
4. **Shot** `t15-2-palette`. Check: a "Create Extension…" row. `$E key $VM Return`.
   Wait: `sleep 2`.
5. **Shot** `t15-2-picker`. Check: a picker titled "Create an extension with…"
   listing "Claude Code", "Codex" and "Kiro CLI".
6. `$E key $VM Return` (the first row). Then `$E g $VM wait 'AGENT-CLI' 60`.
7. Run `$E g $VM screens` and quote the four `AGENT-` lines. Check:
   `AGENT-STUB name=claude`; `AGENT-CWD` ends in
   `/.local/state/muxy-vm/extension-workspace`; `AGENT-CLI ok-42` (the agent
   can run `muxy`).
8. **Shot** `t15-2-agent`. Check: a new terminal tab shows those lines.
9. Run `$E g $VM exists '~/.local/state/muxy-vm/extension-workspace/AGENTS.md'`.
   Check: `yes`.
10. End the stub: run `$E g $VM find 'AGENT-STUB'`, note the id before the colon,
    run `$E g $VM muxy session end <id> --yes`, then `$E g $VM gone 'AGENT-STUB'`.

## T15.3 Without the muxy command, Create Extension asks to install it

Linux only. On macOS mark both parts `SKIP` and go to T15.4.

1. Run `$E g $VM quit`, then `$E g $VM launch-without-muxy`. Check: it prints
   `app running without muxy on its PATH`.
2. Run `$E g $VM exists '~/.local/bin/muxy'`. Check: `no`.
3. Press palette `$E press $VM palette`, `$E type $VM 'create extension'`,
   `$E key $VM Return`. Wait: `sleep 2`. `$E key $VM Return` (Claude Code).
   Wait: `sleep 3`.
4. **Shot** `t15-3-ask`. Check: a dialog "Install the muxy Command?" saying agents
   use the muxy command and that it will be linked at `~/.local/bin/muxy`, with
   "Install and Continue" and "Cancel".
5. Click Cancel: `$E click-element $VM Cancel`. Wait: `sleep 2`.
6. Run `$E g $VM exists '~/.local/bin/muxy'`. Check: `no`. Run
   `$E g $VM find 'AGENT-STUB'`. Check: exit 1 (no agent started).
7. Repeat step 3, then click `$E click-element $VM 'Install and Continue'`.
   Then `$E g $VM wait 'AGENT-CLI' 60`.
8. Run `$E g $VM exists '~/.local/bin/muxy'`. Check: `yes`.
9. Run `$E g $VM screens`. Check: `AGENT-MUXY` ends in `/.local/bin/muxy` and
   `AGENT-CLI ok-42`. **Shot** `t15-3-installed`.
10. End the stub as in T15.2 step 10, then run `$E g $VM agent-fixture-remove`.

## T15.4 A language pack is installed and chosen

1. Run `$E g $VM agent-fixture-remove` (harmless if already removed), then
   `$E g $VM quit`.
2. Run `$E g $VM symbol-language-fixture`. Check: it prints
   `resource-only French symbol fixture installed and selected`.
3. Run `$E g $VM launch`.
4. Run `$E g $VM config settings.toml`. Check: a line
   `language = "e2e-symbol-language:fr"`.
5. Press settings `$E press $VM settings`, press search `$E press $VM find`,
   `$E type $VM 'app language'`. Wait: `sleep 1`.
6. **Shot** `t15-4-settings`. Check: an "App language" row showing
   "French symbols fixture".

Needs verification before first use:
- T15.1: the commit chip appears only when a provider is installed and the
  project has changes; confirm `Commit --role button` is a single match on Linux and that
  the dialog's cancel button is named "Cancel" on macOS and Linux.
- T15.2: the picker's row order (installed providers, last used first) and
  whether Return on the picker starts the selected row.
- T15.3: Linux only because the fixture is; on Linux the debug build is not
  in `/tmp`, so `installed_tool` succeeds and the app should ask to install.
- T15.4: the pack's translated text, "Actions for Dossier", is on a hidden
  status-bar button that `swiftvm control look` lists on Wayland but not on
  X11 or macOS, so the test checks the saved choice and the App language row only.

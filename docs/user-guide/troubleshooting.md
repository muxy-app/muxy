# Troubleshooting

If something goes wrong, this page collects the common fixes. If your issue isn't here, please [open an issue](https://github.com/muxy-app/muxy/issues).

## Logs

Muxy writes logs through the unified macOS logging system. Stream them live:

```bash
log stream --predicate 'subsystem == "app.muxy"' --info --debug
```

Or grab a recent slice:

```bash
log show --predicate 'subsystem == "app.muxy"' --last 10m --info --debug
```

## Terminal is blank or unresponsive

- Try **Muxy → Reload Configuration** (`Cmd+Shift+R`).
- Check `~/Library/Application Support/Muxy/ghostty.conf` parses by opening it in **Open Configuration...**.
- If the issue is reproducible, check `log stream` while reproducing.

## "muxy" CLI not found

Run **Muxy -> Install CLI** from the menu. Muxy first tries `/usr/local/bin/muxy`, then falls back to `~/bin/muxy` or `~/.local/bin/muxy` if needed. Make sure the installed directory is on your `$PATH`.

## Project won't open via `muxy <path>`

The path must exist and must be a directory (not a file). Relative paths are resolved against the shell's current directory. Quote paths with spaces.

## Pull request actions disabled

Pull request features require the `gh` CLI to be installed and authenticated:

```bash
brew install gh
gh auth login
```

After authenticating, restart Muxy so it picks up the new credentials.

## Commit or Create PR is disabled

- **Commit** requires an active branch with uncommitted changes. It is disabled for a clean worktree, detached HEAD, or while another repository action is running.
- **Create PR** appears only when Muxy can confirm through `gh` that the active branch has no pull request. Like **Commit**, it is disabled while the working tree is clean. Install and authenticate `gh` as shown above.
- Install and authenticate at least one supported provider CLI, then reopen Muxy or bring it to the foreground so the provider list refreshes. The dropdown beside each action shows locally missing CLIs.
- Muxy resolves local provider CLIs through your interactive login shell, matching the `PATH` used by a normal terminal session.
- For an SSH workspace, the selected provider CLI and `gh` must be installed and authenticated on the remote host. If **Auto** selects a CLI that is unavailable remotely, choose the installed provider explicitly from the action dropdown.
- Provider CLIs run headlessly and cannot show interactive authentication or permission prompts. Authenticate the chosen CLI in a terminal first, then retry the button; failures are shown in a toast.
- AI only generates metadata. Muxy always owns staging, branch creation, commits, pushes, and pull request creation. Update the prompt when the provider returns invalid JSON or unsuitable metadata, not to change the native Git sequence.
- **Add Prompt** appends one-time instructions after the configured global or project prompt for the current action without changing Settings.

## Mobile server won't start

- Make sure the port (default 4865) isn't in use: `lsof -i :4865`.
- Check **Settings → Mobile** for an error message — port conflicts and bind failures are surfaced there.

## Notifications aren't showing

- Check **Settings → Notifications** that Toast or Desktop notifications are enabled and that the relevant provider integration is on.
- macOS may have suppressed Muxy's system notifications — check **System Settings → Notifications → Muxy**.
- For socket‑based integrations, verify the socket exists: `ls -l ~/Library/Application\ Support/Muxy/muxy.sock`.

## Reset state

If you want to start fresh, quit Muxy and remove:

```
~/Library/Application Support/Muxy/
```

This wipes projects, worktrees, notifications, approved mobile devices, and Muxy's Ghostty config at `~/Library/Application Support/Muxy/ghostty.conf`. Your system Ghostty config at `~/.config/ghostty/config` is left alone.

## Reporting a bug

When filing an issue, include:

- macOS version
- Muxy version (Muxy menu → About Muxy)
- Reproduction steps
- A `log show --predicate 'subsystem == "app.muxy"' --last 10m` snippet if relevant

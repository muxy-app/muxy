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

## Double Shift doesn't open the quick terminal

- Open **Settings → Quick Terminal** and make sure **Enable Quick Terminal** is on.
- Open **Settings → Quick Terminal** and check the Input Monitoring status.
- Enable Muxy under **System Settings → Privacy & Security → Input Monitoring**, then bring Muxy to the foreground so it can retry the listener.
- If access remains unavailable, assign a conventional global shortcut such as Option Space. Conventional shortcuts do not require Input Monitoring.
- Double Shift is intentionally ignored while another key or modifier is involved, which prevents normal capital-letter typing from opening the terminal.

## The quick terminal is not transparent or blurred

- Open **Settings → Quick Terminal** and set Terminal transparency above 0%.
- Raise Background vibrancy above 0% for a progressively stronger native material effect. At 0%, the wallpaper remains sharp.
- macOS Reduce Transparency and Increase Contrast intentionally force an opaque, unblurred terminal. Check both under **System Settings → Accessibility → Display**.

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
- Click **Refresh** beside the provider to restage its hook files under `~/Library/Application Support/Muxy/hooks` and update its configuration.
- Check `~/Library/Application Support/Muxy/hooks.log` for hook delivery failures.
- macOS may have suppressed Muxy's system notifications — check **System Settings → Notifications → Muxy**.
- For socket‑based integrations, verify the socket exists: `ls -l ~/Library/Application\ Support/Muxy/muxy.sock`.
- For AI coding agents, use the per‑provider **Test** button and see [AI notifications](../features/ai-notifications.md) for the hook pipeline and health engine.

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

# Background sessions

Background sessions keep local workspace terminals running after Muxy quits. The feature is off by default.

## Enable or disable

Open **Settings → Terminal → Background Sessions**.

Changing the setting is restart-only. Enabling it explains that Muxy will replace eligible workspace terminal backing after the next Quit and reopen. Existing shells are not adopted or changed in the current process.

On the next fresh launch, Muxy creates or recovers a background session for every eligible local workspace terminal and saves its session ID before creating terminal renderers. Only visible panes receive a renderer. Hidden tabs remain daemon-backed without a Ghostty surface until selected.

Disabling shows the number of sessions that will be ended. After confirmation, the current process keeps its applied mode. On the next fresh launch, Muxy ends every session and descendant before clearing saved session IDs or opening ordinary workspace terminals. If cleanup or persistence fails, startup remains blocked with Retry and Settings or Quit access rather than starting duplicate shells.

Remote terminals and Quick Terminal are always excluded.

## Session and renderer model

A private `muxy-session` daemon owns each real shell PTY. Ghostty runs the bundled `muxy-session attach` client as the renderer-side command. Closing a renderer or quitting Muxy disconnects only that client. The daemon and shell continue running.

Each session allows one active renderer attachment. Selecting a hidden tab or reopening Muxy attaches a new renderer to the same session. The daemon sends up to 256 KiB of recent ordinary output before ordered live output. Older output can be omitted. Alternate-screen output is not retained as ordinary replay.

Muxy does not adopt retained Swift sessions, arbitrary processes, old protocol sessions, or raw `paneSessionID` values. A missing saved session is shown as **Missing/Ended** and is never recreated automatically. **Start New** is an explicit action and does not silently rerun a one-shot startup command.

## Close and Background

Closing a tab ends its persistent session and its tracked process tree before removing the tab. Bulk close actions validate and end every persistent candidate before changing workspace state. They never turn a closed tab into a background session implicitly.

Use **Send to Background** from an eligible local terminal's context menu when you want to remove its workspace tab without ending its processes. This clears workspace placement and drops the renderer while preserving the session.

Ordinary Quit never sends an end operation. Sessions survive the app closing and reconnect to the same shell identity after reopen.

## Session Manager

Select **Terminal Sessions** in the status bar. The manager has three sections:

- **Workspace Sessions** for sessions that still have workspace placement, including hidden tabs without renderers
- **Background Sessions** for sessions without workspace placement
- **Missing/Ended** for saved workspace links whose daemon session is no longer available

Available row actions depend on state:

- **Focus** selects the existing workspace tab.
- **Reattach** restores a background session to its original project and worktree.
- **End** terminates the exact session tree and removes its workspace placement.
- **Start New** replaces an ended or missing link only after an explicit action.
- **Remove** deletes an ended or missing durable link without starting a shell.

The footer provides **End All Sessions** and **Terminal Settings**. Destructive confirmations include the affected session count. The status control supports mouse, Enter, and Space activation, and action rows expose descriptive accessibility labels.

## Process ownership and cleanup

The daemon tracks the shell, process session, process groups, controlling terminal, ancestry, and PID start identities. Ending a session revalidates each exact identity, rescans descendants during a bounded grace period, escalates surviving tracked processes, reaps the direct child, and reports success only after the tracked tree is gone. Muxy never cleans sessions by executable name.

A shell exiting naturally is distinct from an attach-client failure or a daemon connection failure. Muxy removes only the descriptor-confirmed ended session. Connection and renderer failures preserve workspace state and remain retryable.

## Isolation and security

Development and release builds use separate runtime directories, sockets, locks, logs, and protocol build modes. The session leaf is owner-only mode `0700`; its socket, lock, and log are mode `0600`. Runtime paths reject symlinks, foreign ownership, and unsafe permissions. The daemon authenticates same-user peers before decoding protocol frames and never unlinks a live socket.

The helper is bundled at `Contents/MacOS/muxy-session` and is signed before the containing app. zsh, bash, fish, elvish, and nushell use the existing bundled Ghostty shell integrations.

## CLI

Use the retained CLI while Muxy is open:

```bash
muxy list-sessions
muxy kill-session --session <session-id>
```

`list-sessions` reports workspace placement as the `attached` field. A hidden workspace tab is attached even when it has no renderer. `kill-session` reports success only after exact cleanup and removes any linked workspace tab.

See [Muxy CLI](muxy-cli.md) for the exact columns and command behavior.

## Acceptance status

Automated headless tests cover protocol limits, path and peer security, daemon lifecycle, renderer replacement, replay bounds, process-tree cleanup, restart transitions, workspace actions, CLI compatibility, and debug and release bundle structure. Automated verification never launches Muxy or a staged app bundle.

Manual native acceptance remains pending for complete app lifecycle behavior, visual presentation, VoiceOver announcements, real theme and scaling combinations, interactive confirmations, and daily use. Ask the user to perform these checks and record only behavior they report observing.

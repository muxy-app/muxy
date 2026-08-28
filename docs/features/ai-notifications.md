# AI notifications

The Rust app accepts Agent Hook v3 events from Antigravity CLI, Claude Code, Codex, Cursor, GitHub Copilot, Droid, Grok, Kiro CLI, OpenCode, Pi, and Xal through the Unix socket. P2 owns the wire protocol, acknowledgement, deduplication, and initial pane resolution. P5 consumes the resulting typed event and routes known-provider notifications through the same history and delivery coordinator as legacy socket and terminal OSC notifications.

The current Rust implementation does not install provider hooks, mutate provider configuration, track provider health, or update agent lifecycle UI. Those client and integration features remain P11 work. The provider Test and Refresh controls do not execute hook installation or repair in P5.

## Protocol v3

Hooks send one newline-delimited JSON object to `~/Library/Application Support/Muxy/muxy.sock` or `muxy-dev.sock` for a debug build:

```json
{"v":3,"kind":"agent_event","id":"…","provider":"codex","paneID":"…","phase":"finished","title":"Codex","body":"Done","pids":[],"ts":1721234567}
```

- `id` identifies one logical delivery. A retry must resend the same line and ID.
- `provider` must match one of the eleven provider IDs in Muxy's shared provider catalog before P5 creates notification history.
- `paneID` is optional. When it is absent, P2 can resolve the first supplied process ancestor in `pids` to a pane.
- `phase` is `working`, `waiting`, or `finished`.
- `test: true` marks a synthetic notification event.

The server replies to a valid envelope with:

```json
{"kind":"ack","ok":true,"v":3}
```

The acknowledgement is sent before duplicate suppression and app delivery. The server remembers the 256 most recently applied non-empty IDs. A duplicate is acknowledged again but does not create another notification. A missing or empty ID is not deduplicated.

Events with the wrong version, wrong kind, unsupported phase, empty provider, or malformed pane ID are rejected without an acknowledgement. The bridge-side one-mebibyte input cap and 400 ms delivery budget belong to the future P11 client implementation, not the P5 receiver.

## Target resolution

Normal events require a known provider and a live resolved terminal pane. An explicit or process-matched pane that has become stale is dropped and never falls back to another tab. An unresolved normal event is also dropped.

A Test event uses its live explicit pane when supplied. If its pane ID is absent or malformed, it may fall back to the active terminal context. A valid stale Test pane is dropped. Test titles default to `Notifications`.

A normal event with empty title and body creates no notification. If only the title is empty, it defaults to `Task completed!`. P5 uses processing time for history ordering; the envelope timestamp remains diagnostic protocol data.

Every accepted record captures complete project, worktree, area, root-tab, pane, and worktree-path identity before any store or presentation mutation starts.

## History and delivery

Accepted AI events enter the shared newest-first notification history. The store retains at most 200 records in the selected Rust profile's private `notifications.json`. It tolerates malformed sibling rows, writes atomically with mode 0600, debounces ordinary saves for two seconds, and flushes synchronously when the main window closes or the app quits. Loaded history is retained and marked read at startup. The Rust app does not import the retained Swift notification file.

Delivery settings are independent:

- Notification toasts use the latest-replacing three-second toast presentation.
- macOS desktop delivery is optional. Ordinary delivery never prompts for authorization; enabling the setting owns the permission request.
- Sound uses one of the fourteen named macOS system sounds. `None` and unknown names are silent.
- Generic worktree and repository feedback may use the same toast presentation but never enters notification history, desktop delivery, sound, coalescing, or unread state.

The sidebar footer bell opens the notification panel. Rows show source, title, body, relative time, and unread state. Opening a live row navigates by stable project, worktree, area, root-tab, and pane IDs; the stored worktree path is context data only. Opening a stale row recreates nothing but still marks the record read. Project, worktree, root-tab, and omnibox presentation derive unread state from the same store.

## Hook and OSC pairing

When one AI hook and one terminal OSC notification carry the same body and navigation context within two seconds, Muxy may suppress only the second macOS desktop request. Matching titles and the default completion titles are equivalent for this comparison.

Both records remain in history. Both toasts and both sounds remain. Suppression occurs only when exactly one complementary pending event matches. Ambiguous candidates, different providers, target mismatches, body mismatches, and events outside the two-second window are delivered independently. Legacy socket and future extension-origin notifications are not pairing candidates.

## P11 boundary

P11 remains responsible for:

- the `muxy-hook` client executable and provider-specific shims or plugins
- declarative installation, verification, repair, and obsolete-resource cleanup
- provider configuration mutation permits and development-mode safety
- foreground-process detection and agent lifecycle status
- provider health, Test, and Refresh behavior
- hook staging paths and `MUXY_HOOK_BIN` or `MUXY_HOOK_SCRIPT` terminal variables

P5 adds none of those behaviors. It consumes only Agent Hook v3 envelopes already accepted by the P2 socket server.

## Troubleshooting

- **No acknowledgement.** Check the socket path and validate the v3 envelope, provider, phase, and pane ID syntax.
- **Acknowledged but no history row.** Confirm the provider ID is known and the resolved pane is still live. Empty normal title and body are intentionally dropped.
- **Duplicate delivery missing.** Reusing a non-empty event ID is expected to suppress the duplicate after acknowledging it.
- **Socket missing.** Check `ls -l ~/Library/Application\ Support/Muxy/muxy.sock` or the debug `muxy-dev.sock` path.
- **Provider integration unavailable.** Automatic hook installation and repair are not implemented until P11.

See [Terminal notifications](terminal.md) for OSC 9 and OSC 777 behavior and [Socket protocol](../development/socket-protocol.md) for framing and routing details.

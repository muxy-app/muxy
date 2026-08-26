# Swift profile migration

Muxy release performs one bounded import from the retained macOS Swift profile into the Rust-owned profile.

## Locations

| Purpose | Location |
|---|---|
| Swift source | `~/Library/Application Support/Muxy` |
| Rust release root | `~/.muxy` |
| Rust debug root | `~/.muxy-dev` |
| Portable preferences | `<root>/preferences.json` |
| Migration state | `~/.muxy/swift-profile-migration.json` |

Debug never runs the importer. Recognized staged test executables may inject synthetic roots and sources. Normal applications do not accept storage overrides from the environment.

## Import behavior

Release checks the versioned migration state before inspecting the Swift source. Terminal outcomes return immediately.

The App Support allowlist is:

- `projects.json`
- `recently-removed-projects.json`
- `project-groups.json`
- `workspaces.json`
- `settings.json`
- `ui-scale.json`
- `keybindings.json`
- `command-shortcuts.json`
- `editor-settings.json`
- `quick-terminal-shortcut.json`
- `approved-devices.json`
- `remote-devices.json`
- `browser-profiles.json`
- `ghostty.conf`
- `worktrees/`
- `logos/`

Existing destination entries always win. Missing entries are reported but are not errors. Directory merges recurse into missing entries. Symlinks, sockets, devices, and other non-regular entries are not followed or copied.

Every new regular file is copied to a uniquely created destination-side temporary file and published without replacing an entry created during the copy. Copy failures remove only temporary files owned by that attempt. The importer never modifies, moves, or deletes the Swift source.

## Preferences

Normal Rust preference reads and writes use private atomic `preferences.json` beneath the selected root. Rust does not read a preferences plist directly.

During an eligible pending release migration on macOS, a narrow Foundation reader imports only the approved production `NSUserDefaults` keys. Existing keys in `preferences.json` win. Debug, Linux, completed migrations, source-missing migrations, abandoned migrations, and ordinary tests do not read the production suite.

## Outcomes

| Outcome | Startup behavior | Future Swift inspection |
|---|---|---|
| `completed` | Continue | Never |
| `source_missing` | Continue | Never |
| `pending` after first failure | Stop with an actionable error | Retry once on the next launch |
| `abandoned` after second failure | Continue with imported data and Rust defaults for missing values | Never |

The state records paths and error categories, not user values or copied contents. An unreadable, malformed, or unsupported state blocks startup without inspecting the Swift source. A root-local file lock prevents concurrent migration attempts.

## Verification

Run:

```bash
scripts/verify-p2-5-migration.sh --self-test
scripts/verify-p2-5-migration.sh
```

The full verifier stages `com.muxy.tests` debug and release bundles under `target/test-verification`, uses an isolated `HOME`, hashes synthetic sources, and proves success, target-wins merge, source-missing completion, retry, abandonment, and terminal no-reinspection without touching real user state.

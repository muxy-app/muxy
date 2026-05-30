# Low Memory Mode — Review Fixes

Three issues from code review: broken `%output` parsing, per-keystroke process spawning, and missing architecture documentation.

## 1. Fix `parseControlOutput` Octal Escape Parsing

**Problem:** `TmuxCaptureService.parseControlOutput` assumes tmux `%output` payloads are base64 encoded. The tmux man page states that value uses octal `\xxx` escaping for non-printable characters and backslash. This causes base64 decode to silently fail, returning empty data — remote/mobile terminals appear blank or stale.

**Fix:** Rewrite the parser to scan for `\xxx` (backslash + 3 octal digits) and convert to bytes. All other characters pass through literally.

**Scope:**
- `TmuxCaptureService.parseControlOutput(_:)` — replace base64 decode with octal escape decoder
- `TmuxControlOutputParsingTests` — rewrite all tests to use real tmux-format payloads (`%output %0 Hello\015\012`) instead of fabricated base64

## 2. Route Commands Through Control Mode Stdin

**Problem:** `sendInput`, `resizeSession`, and `scroll` each spawn a new `Process()` (fork+exec) per call. Rapid typing produces 10-30 process spawns per second.

**Fix:** The `TmuxControlModeProcess` already runs a persistent tmux control mode connection per streaming pane. Route commands through its stdin instead of spawning CLI processes.

| Operation | Current | Proposed |
|---|---|---|
| Input | `Process()` + `tmux send-keys` | `send-keys -t %0 -l text\n` via stdin |
| Resize | `Process()` + `tmux resize-window` | `refresh-client -C WxH\n` via stdin |
| Scroll | `Process()` + `tmux send-keys -N` | `send-keys -t %0 -N N Up\n` via stdin |

**Changes:**
- `TmuxControlModeProcess` — add `send(_ command: String)` method that writes to the process's stdin pipe, thread-safe via existing `lock`
- `TmuxCaptureService` — `sendInput`/`resizeSession`/`scroll` check for a running streaming process first; fall back to CLI if none exists (non-streaming panes)
- Remove `DispatchQueue.global` + `Process()` + `waitUntilExit()` from the three methods when control mode path is used

**Fallback:** Non-streaming panes (local-only, not viewed remotely) continue using CLI processes. This is acceptable because those are user-driven and naturally rate-limited.

## 3. Architecture Documentation

**Problem:** Reviewer flagged unclear tmux dependency contract and lack of "why" documentation.

**Fix:** Add doc comments at key decision points:
- `TmuxConfiguration` — why tmux, graceful degradation without it, minimum version (3.3+ for control mode `%output`)
- `TmuxCaptureService` — two codepaths (control mode stdin vs CLI fallback), `%output` protocol format
- No new files or structural changes

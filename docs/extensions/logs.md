# Extension Logs

Every loaded extension gets its own log file at:

```
~/.config/muxy/extensions/<name>/logs/output.log
```

Subprocess stdout/stderr is redirected directly to that file. Webview tabs and `runScript` commands' `console.log` / `console.warn` / `console.error` calls also feed the same file via the JS bridge, prefixed with `[log]`, `[warn]`, or `[err]`.

## Viewing logs

Three surfaces:

- **Settings → Extensions → Show Logs**: inline tail of the last 200 lines, plus a "Reveal Log File" button.
- **Bottom-dock Extension Output panel**: click the `ext output` chip in the status bar to open. Drop-down picks the extension; the file is tailed live via a file system event source — no polling, no in-memory buffer.
- **Open the file directly** in any editor: it is a plain UTF-8 newline-separated text file.

## From inside an extension

```js
console.log('hello', { count: panes.length });
console.warn('the pane title is suspicious:', pane.title);
console.error('failed to do thing', err);
```

These work identically in both webview tabs and `runScript` JS contexts. The output is appended to the extension's `output.log`.

From the subprocess, anything you write to stdout or stderr also lands there:

```bash
echo "[my-ext] started" 1>&2
```

You can also write to it directly using the `MUXY_EXTENSION_LOG` environment variable that Muxy sets when spawning the subprocess:

```bash
printf '[ping] %s\n' "$(date)" >> "$MUXY_EXTENSION_LOG"
```

## Size and rotation

- Cap: **5 MB** per file.
- Trim policy: a background pass every **10 minutes** checks each extension's log file. Any file over 5 MB is trimmed in place to roughly the most recent 1.25 MB (last ~25%). The trim is line-aligned — the oldest lines are dropped, the most recent are preserved.
- No mtime/checkpoint state is persisted; the pass uses the live file size and the most-recent modification time as hints.

If you need finer control, write to your own file alongside `output.log`. Muxy does not manage other files in the `logs/` directory.

## Format

```
[muxy] started my-ext v0.1.0
[muxy] exited cleanly
[log] {"openedFrom":"palette"}
[warn] retrying connection
[err] Error: permission denied (panes:write)
    at handle (script.js:14:5)
```

The `[muxy]` prefix is reserved for lifecycle events emitted by Muxy itself. Everything else is your extension's output. Timestamps are not added today — emit them yourself if you need them.

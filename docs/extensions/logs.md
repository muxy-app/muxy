# Extension Logs

Every loaded extension gets its own log file:

```
~/.config/muxy/extensions/<name>/logs/output.log
```

The background host's stdout/stderr is captured into this file. In pages and `runScript` contexts, `console.log` / `console.warn` / `console.error` also feed it via the JS bridge, tagged `[log]`, `[warn]`, or `[err]`.

## Viewing logs

Three surfaces:

- **Settings → Extensions → Show Logs**: inline tail of the last 200 lines, plus a "Reveal Log File" button.
- **Bottom-dock Extension Output panel**: click the `ext output` chip in the status bar, then pick the extension. The file is live-tailed via a file system event source — no polling.
- **Open the file directly** in any editor — it is plain UTF-8 text.

## Logging from an extension

In pages and `runScript` JS contexts, use `console.*`:

```js
console.log('hello', { count: panes.length });
console.warn('the pane title is suspicious:', pane.title);
console.error('failed to do thing', err);
```

In `background.js`, the same `console.*` calls work, and anything the host writes to stdout/stderr lands in the file too.

## Size and rotation

- Cap: **5 MB** per file.
- A background pass every **10 minutes** trims any file over the cap in place, down to roughly the most recent 1.25 MB. The trim is line-aligned — oldest lines are dropped, newest preserved.

To keep something Muxy won't trim, write to your own file alongside `output.log`; Muxy does not manage other files in `logs/`.

## Format

```
[muxy] started my-ext v0.1.0
[muxy] exited cleanly
[log] {"openedFrom":"palette"}
[warn] retrying connection
[err] Error: permission denied (panes:write)
    at handle (script.js:14:5)
```

The `[muxy]` prefix is reserved for lifecycle events emitted by Muxy itself. Timestamps are not added — emit them yourself if you need them.

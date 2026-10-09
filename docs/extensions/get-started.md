# Get started

This page builds a working extension in a few minutes: a panel you toggle from
the top bar or a shortcut.

## 1. Create the files

An extension is a folder with a `package.json`. Muxy loads the folder's `dist/`
build output when it has one, otherwise the folder itself, so a plain HTML page
needs no build step.

```
hello/
  package.json
  panel.html
```

`package.json`:

```json
{
  "name": "hello",
  "version": "0.1.0",
  "muxy": {
    "description": "Says hello",
    "permissions": ["panels:write", "notifications:write"],
    "panels": [{ "id": "hello", "title": "Hello", "entry": "panel.html" }],
    "commands": [
      {
        "id": "toggle",
        "title": "Hello: Toggle Panel",
        "action": { "kind": "togglePanel", "panel": "hello" },
        "defaultShortcut": "ctrl+opt+h"
      }
    ],
    "topbarItems": [
      { "id": "hello", "icon": "sparkles", "command": "toggle", "tooltip": "Hello" }
    ]
  }
}
```

`panel.html`:

```html
<!doctype html>
<body style="background: var(--muxy-background); color: var(--muxy-foreground)">
  <button id="hi">Say hello</button>
  <script>
    document.getElementById("hi").onclick = () =>
      muxy.notifications.notify({ title: "Hello", body: "From my extension" });
  </script>
</body>
```

The extension `name` should use lowercase letters, digits, `.`, and `-`. Its
pages don't load otherwise.

## 2. Load it

1. Open **Settings → Extensions** and choose **Load Unpacked…**.
2. Pick the `hello` folder.
3. Open its entry and turn it on. Extensions start disabled.
4. Click the sparkles icon in the top bar, or press `Ctrl+Opt+H`.

The extension shows an **Unpacked** badge. After editing, choose **Reload**.
**Unload folder** removes it from Muxy and keeps your files.

## 3. Use a build tool

For anything bigger, use any web tooling, such as Vite. Point every `entry` at
paths inside your build output, and copy `package.json` into `dist/` as part of
the build. Only `dist/` is published.

```json
"scripts": {
  "build": "vite build && cp package.json dist/"
}
```

## Debugging

- `console.log`, `console.warn`, and `console.error` from pages and scripts go
  to the extension's log, along with uncaught errors.
- The extension's page in **Settings → Extensions** shows recent log lines,
  load errors, and **Reveal log**.
- The log file is `logs/output.log` inside the extension's loaded folder.

## Next

- Declare what it does in the [manifest](manifest.md).
- Ask for the minimum [permissions](permissions.md).
- Add [tabs](tabs.md), [panels](panels.md), [bar items](bar-items.md), and
  [commands](commands.md).
- Work with [files](files.md), [Git](git.md), and [events](events.md).
- [Publish it](contributing.md).

## Coming from 1.x

Most 1.x extensions work unchanged. The differences:

- **Install folder.** Extensions live in Muxy 2's profile folder, not
  `~/.config/muxy/extensions`. Importing the installed 1.x copies them over.
- **No scaffold.** There is no **Create** button or starter kit. Use
  **Load Unpacked…**.
- **Lowercase names.** Pages only load for names made of `a-z`, `0-9`, `.`,
  and `-`.
- **Background scripts run in-process,** in JavaScriptCore on their own thread.
- **Not supported yet:** the built-in browser API (`muxy.browser`, `browser`
  tabs), remote methods for the phone app, home views, reading extension
  settings at runtime, and the `worktree.offline` event.
- **Bar items** always sit in the top bar. There is no icon rail.
- **New:** the `openPanel` command action, `visible` on panel header buttons,
  `execAsync` on pages and scripts, and the `modal.opened` and `modal.closed`
  events.

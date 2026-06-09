# Get started

> **Status:** under active development. Marked **DEV** in **Settings → Extensions**. The manifest format, permission set, and wire format may change without notice.

The fastest way to understand Muxy extensions is to make one run. This page takes
you from nothing to a live extension in about two minutes, then points you at the
rest of the docs.

An extension is an npm + [Vite](https://vitejs.dev) project that Muxy loads on
launch. It can add UI (tabs, panels, popovers, topbar and status-bar items),
register palette commands, react to workspace events, and — with permission —
drive the same verbs the `muxy` CLI exposes.

## Run your first extension

1. In Muxy, open the **Extensions** modal → **Create**, pick a folder, and name
   it `my-extension`. Muxy scaffolds the
   [`vanilla`](https://github.com/muxy-app/muxy/tree/main/Muxy/Resources/starter-kits/vanilla)
   starter kit (a Hello panel, a topbar icon, and a command) and loads it as a
   dev extension automatically.
2. Build it so Muxy has a `dist/` to read:

   ```bash
   cd my-extension
   npm install
   npm run dev      # rebuilds on every change
   ```
3. Click **Reload** in the Extensions modal, then press **⌘⇧H** (or use the
   topbar **sparkles** icon). The **Hello** panel toggles — that's your extension
   running.

Dev extensions show a **DEV** badge. Edit the source, then **Reload** to see
changes. **Remove from Muxy** on the detail page unloads it without touching your
folder on disk.

## Make it your own

The starter kit is a working example of the most common pieces. To build
something real:

- **Declare what it does** in `package.json` under the `muxy` key — see
  [Manifest](manifest.md).
- **Add UI** with [Tabs](tabs.md), [Panels](panels.md), [Popovers](popovers.md),
  [Sidebars](sidebars.md), [Topbar](topbar.md), and [Status Bar](statusbar.md)
  items, or run logic from [Palette Commands](palette-commands.md) and
  [Scripts](scripts.md).
- **Work with the workspace** through [Events](events.md), [Git](git.md),
  [Files](files.md), [HTTP](http.md), and [Settings](settings.md).
- **Request the minimum** you need — see [Permissions](permissions.md).

When you want the full picture of how extensions load and run, read the
[Overview](overview.md).

## Publish it

Ready to share it with everyone? [Contributing an extension](contributing.md)
walks through the fork → develop → validate → pull request flow.

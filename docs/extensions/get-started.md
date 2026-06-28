# Get started

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
   npm run build    # produces dist/ — the folder Muxy loads
   ```

   `npm run dev` starts Vite's dev server for fast iteration, but Muxy loads
   the built `dist/` folder, so run `npm run build` (and **Reload**) to see
   changes in the app.
3. Click **Reload** in the Extensions modal, then press **⌘⇧H** (or use the
   topbar **sparkles** icon). The **Hello** panel toggles — that's your extension
   running.

Dev extensions show a **DEV** badge. Edit the source, then **Reload** to see
changes. **Remove from Muxy** on the detail page unloads it without touching your
folder on disk.

## Coding-agent skill

Scaffolded extensions already include the `muxy-extension` skill (in
`.claude/skills/` and `.agents/skills/`) so coding agents follow Muxy's
conventions. To add it to an existing project, install it from
[skills.sh](https://www.skills.sh):

```bash
npx skills add github.com/muxy-app/muxy/tree/main/Muxy/Resources/skills/muxy-extension
```

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

# Contributing an extension

This guide walks you through creating, validating, and publishing a Muxy extension.

The reference material for authors lives here in this docs set; the manifest schema lives
alongside it in this repository, and a working example lives in the extensions repository:

- Example extension: [`extensions/git`](https://github.com/muxy-app/extensions/tree/main/extensions/git)
- Manifest schema: [`schema/manifest.schema.json`](schema/manifest.schema.json)

Published community extensions are hosted in the separate
[`muxy-app/extensions`](https://github.com/muxy-app/extensions) repository, which carries
the validation, packaging, and publishing tooling. You open a pull request there to
ship an extension to everyone.

## Prerequisites

- [Node.js](https://nodejs.org) 18 or newer (npm comes with it).
- A Muxy installation to test against.

## 1. Start from a starter kit

The fastest path is the Muxy **Extensions** modal → **Create**: Muxy scaffolds the
[`vanilla`](https://github.com/muxy-app/muxy/tree/main/Muxy/Resources/starter-kits/vanilla)
starter kit into `~/.config/muxy/extensions/<name>` for you.

To start by hand, copy that kit and rename the directory to match the package `name`. It is
a minimal npm + [Vite](https://vitejs.dev) project (one panel, a topbar item, a command) in
vanilla JavaScript with [Tailwind CSS](https://tailwindcss.com), themed against the app's
`--muxy-*` tokens.

For a full-featured reference, see the [`git`](https://github.com/muxy-app/extensions/tree/main/extensions/git) extension.

## 2. Edit `package.json`

Open `my-extension/package.json`. Keep `name` (matching the directory) and `version` at
the top level, and put every Muxy manifest field under the `muxy` key. Your `package.json`
must declare a `build` script — the publishing pipeline runs `npm run build` and ships the
`dist/` it produces.

```json
{
  "name": "my-extension",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": { "dev": "vite", "build": "vite build" },
  "devDependencies": { "vite": "^5.0.0" },
  "muxy": {
    "description": "What it does",
    "permissions": ["notifications:write"],
    "commands": [{ "id": "ping", "title": "My Extension: Ping" }]
  }
}
```

See the [manifest reference](manifest.md) for every option, and the
[schema](schema/manifest.schema.json) for the authoritative contract.

## 3. Build your UI

Extensions are bundled with Vite, so use any npm packages and any framework — React, Vue,
Svelte, or plain HTML/CSS/JS. Entry/asset paths in `muxy` (popover/tab `entry`,
`background`, marketplace `icon`/`screenshots`) resolve against the build output, so make
sure `vite build` emits them into `dist/`. The example includes a tab; adapt it or add
panels, popovers, palette commands, and more. See the rest of this docs set for each
surface:

- [Overview](overview.md) — architecture, lifecycle, security model
- [Permissions](permissions.md) — request the minimum you need
- [Events](events.md), [Tabs](tabs.md), [Panels](panels.md), [Popovers](popovers.md)
- [Palette commands](palette-commands.md), [Topbar](topbar.md), [Status bar](statusbar.md)
- [Settings](settings.md), [Scripts](scripts.md), [Logs](logs.md)

Install dependencies and produce a build:

```bash
cd my-extension
npm install
npm run build
```

## 4. Develop in place (Load Unpacked)

You don't need to copy your project into Muxy's config folder. In the **Extensions** modal,
click **Load Unpacked** and pick your project folder — your git checkout works directly.
Edit, then click **Reload** to pick up changes. (The **Create** modal can also scaffold a
new extension straight into a folder you choose; pick a location outside the config folder
and it's loaded as a dev extension automatically.)

Muxy reads from the `dist/` build output when it exists, otherwise from the folder you
picked. So:

- **Plain HTML/JS** (no build) — Muxy reads your source directly; just **Reload** after edits.
- **npm / framework** (React, Vue, Svelte) — run `npm run dev` (or `npm run build`) yourself
  to produce `dist/`; Muxy reads that. Muxy never runs npm for you.

Dev extensions show a **DEV** badge. Use **Remove from Muxy** on the detail page to stop
loading one — your folder on disk is left untouched. Because the checkout *is* the install,
publishing is just pushing that same folder to your fork (next step).

## 5. Validate and publish

To publish, fork the
[`muxy-app/extensions`](https://github.com/muxy-app/extensions) repository, drop your
extension into `extensions/<your-extension>/`, and run its tooling:

```bash
npm install
npm run validate
npm run build
```

Then open a pull request against that repository. CI installs, builds (`npm run build`),
and validates every submission; once a maintainer approves and merges, the publish workflow
runs the build, signs the resulting `dist/`, and releases your extension.

## Style and quality

- Keep bundles small. Avoid heavy frameworks where vanilla JS will do.
- Respect the user. Request the minimum permissions you need.
- Test on the latest Muxy release.

## Questions?

Open a discussion or issue. We're happy to help.

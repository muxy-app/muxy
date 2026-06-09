# Contributing an extension

A step-by-step guide: fork and check out the extensions repo, develop your
extension live in Muxy, then publish it with a pull request.

You author **source** in a sparse checkout of your fork of the
[`muxy-app/extensions`](https://github.com/muxy-app/extensions) repo and develop
it live with **Load Unpacked**. Publishing is just pushing that same folder and
opening a PR — CI builds, signs, and lists it.

- Example extension: [`extensions/git`](https://github.com/muxy-app/extensions/tree/main/extensions/git)
- Manifest reference: [manifest.md](manifest.md) · [schema](schema/manifest.schema.json)

## Prerequisites

- [Node.js](https://nodejs.org) 20 or newer (npm comes with it).
- [git](https://git-scm.com) 2.27 or newer (for `sparse-checkout`).
- A Muxy installation to test against.

## 1. Fork and check out only your extension

You publish through a pull request from your own fork, so fork
[`muxy-app/extensions`](https://github.com/muxy-app/extensions) on GitHub first.

The repo holds every published extension and keeps growing, so never full-clone
it. A **partial + sparse** checkout of your fork downloads just the tooling and
the one folder you work on:

```bash
git clone --filter=blob:none --sparse https://github.com/<you>/extensions
cd extensions
git remote add upstream https://github.com/muxy-app/extensions
git sparse-checkout set extensions/my-extension scripts
```

`--filter=blob:none` skips file contents until they're needed; `--sparse` checks
out an empty tree; `sparse-checkout set` populates only `scripts/` (the publish
tooling) and your `extensions/my-extension/` folder. To work on another extension
later, run `git sparse-checkout set extensions/<other> scripts`.

## 2. Scaffold your extension

In Muxy, open the **Extensions** modal → **Create**, set the **location** to the
`extensions/` folder inside your checkout, and name it `my-extension`. Muxy copies
the [`vanilla`](https://github.com/muxy-app/muxy/tree/main/Muxy/Resources/starter-kits/vanilla)
starter kit (a minimal npm + [Vite](https://vitejs.dev) project with one panel, a
topbar item, and a command, themed against the app's `--muxy-*` tokens) into
`extensions/my-extension/` and loads it as a dev extension automatically.

The directory name **must** equal the package `name`. For a full-featured
reference, see the [`git`](https://github.com/muxy-app/extensions/tree/main/extensions/git)
extension.

## 3. Build and develop live

Install dependencies and start Vite from your extension folder:

```bash
cd extensions/my-extension
npm install
npm run dev      # rebuilds dist/ on every change
```

Muxy reads from the `dist/` build output, so keep `npm run dev` running (or run
`npm run build` after edits). After a rebuild, click **Reload** in the Extensions
modal to pick up changes. Dev extensions show a **DEV** badge; **Remove from
Muxy** on the detail page unloads one without touching your folder on disk.

> If you cloned and edited by hand instead of using **Create** in step 2, use
> **Load Unpacked** and pick `extensions/my-extension/` to load it.

Edit `package.json` to declare what your extension does. Keep `name` (matching the
directory) and `version` at the top level; put every Muxy manifest field under the
`muxy` key. `package.json` must declare a `build` script — the publish pipeline
runs `npm run build` and ships the `dist/` it produces.

```json
{
  "name": "my-extension",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": { "dev": "vite", "build": "vite build" },
  "devDependencies": { "vite": "^5.0.0" },
  "muxy": {
    "$schema": "https://raw.githubusercontent.com/muxy-app/muxy/main/docs/extensions/schema/manifest.schema.json",
    "description": "What it does",
    "permissions": ["notifications:write"],
    "commands": [{ "id": "ping", "title": "My Extension: Ping" }]
  }
}
```

Build any UI you like — React, Vue, Svelte, or plain HTML/CSS/JS — as long as
`vite build` emits your entry/asset paths into `dist/`. See the rest of this docs
set for each surface:

- [Overview](overview.md) — architecture, lifecycle, security model
- [Permissions](permissions.md) — request the minimum you need
- [Events](events.md), [Tabs](tabs.md), [Panels](panels.md), [Popovers](popovers.md)
- [Palette commands](palette-commands.md), [Topbar](topbar.md), [Status bar](statusbar.md)
- [Settings](settings.md), [Scripts](scripts.md), [Logs](logs.md)

## 4. Add the marketplace listing

A `marketplace` block under `muxy` is required to publish — CI rejects PRs without
a listing **icon** and **at least one screenshot**.

```json
"marketplace": {
  "author": "Your Name",
  "github": "your-handle",
  "categories": ["productivity"],
  "icon": "assets/icon.svg",
  "screenshots": ["assets/screenshot-1.png"]
}
```

- **Icon** — SVG (preferred, ≤ 512 KB) or square PNG ≥ 256×256 (≤ 1 MB).
- **Screenshots** — PNG, exactly 1600×1000 (16:10), 1 to 6, each ≤ 3 MB.

Also add a `README.md` to your folder with a short description and the permissions
you use and why.

## 5. Validate before you publish

From the repo root, run the same checks CI runs:

```bash
cd ../..                                   # back to the repo root
npm install
node scripts/validate.mjs my-extension     # schema, paths, ids, listing
node scripts/pack.mjs --dry-run my-extension   # prove it builds and zips
```

Validation fetches the manifest schema over the network the first time, so you
need a connection. Fix anything it reports — CI enforces the same rules.

## 6. Open a pull request

Commit your **source** (the pipeline builds `dist/` for you — it's gitignored) and
push to your fork:

```bash
git add extensions/my-extension
git commit -m "Add my-extension"
git push
```

Open a pull request from your fork against
[`muxy-app/extensions`](https://github.com/muxy-app/extensions) and fill in the
template. CI validates and builds every submission; once a maintainer approves and
merges, the publish workflow builds, signs the `dist/`, and lists your extension.

Published `name@version` pairs are **immutable** — to ship changes, bump
`version` and open another PR.

## Style and quality

- Keep bundles small. Avoid heavy frameworks where vanilla JS will do.
- Respect the user. Request the minimum permissions you need.
- Ship readable source — minified or obfuscated code is flagged for review.
- Test on the latest Muxy release.

## Questions?

Open a discussion or issue. We're happy to help.

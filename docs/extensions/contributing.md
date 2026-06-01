# Contributing an extension

This guide walks you through creating, validating, and publishing a Muxy extension.

The reference material for authors lives here in this docs set; the example and the
manifest schema live alongside it in this repository:

- Example extension: [`examples/hello-world`](examples/hello-world)
- Manifest schema: [`schema/manifest.schema.json`](schema/manifest.schema.json)

Published community extensions are hosted in the separate
[`muxy-app/extensions`](https://github.com/muxy-app/extensions) repository, which carries
the validation, packaging, and publishing tooling. You open a pull request there to
ship an extension to everyone.

## Prerequisites

- [Node.js](https://nodejs.org) 18 or newer (npm comes with it).
- A Muxy installation to test against.

## 1. Start from the example

Copy the example as your starting point. An extension is an npm + [Vite](https://vitejs.dev)
project — the directory name must match the package `name`:

```bash
cp -r examples/hello-world my-extension
```

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

## 4. Test in Muxy

Put your project in `~/.config/muxy/extensions/<name>/` (the directory name must match the
package `name`) and run `npm run build`. Muxy loads from the `dist/` build output when it
exists, so you can keep your source alongside it and just rebuild to pick up changes.

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

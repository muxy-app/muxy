# hello-world

A minimal Muxy extension you can copy as a starting point. It registers one
palette command, **Hello World: Open**, which opens a theme-aware tab with a
button that fires a toast notification.

Extensions are now plain npm + [Vite](https://vitejs.dev) projects. The old
`manifest.json` fields live under the `"muxy"` key in `package.json`, and the
project builds to `dist/` — the `dist/` output *is* the installed extension.

## Files

- `package.json` — npm metadata plus the `"muxy"` block (description,
  permissions, tab types, commands, and listing metadata). All paths inside
  `"muxy"` resolve against the build output (`dist/`).
- `vite.config.js` — builds `tabs/index.html` and copies `public/assets/`
  into `dist/`.
- `tabs/index.html` — the tab UI, using the injected `window.muxy` bridge.
- `tabs/styles.css` — styling driven entirely by Muxy theme variables.
- `public/assets/icon.svg` — the required listing icon (copied verbatim to
  `dist/assets/icon.svg`).
- `public/assets/screenshot-1.png` — the required listing screenshot
  (1600×1000, copied verbatim to `dist/assets/screenshot-1.png`).

## Build it

```sh
npm install
npm run build
```

This emits the installable extension into `dist/`. Use `npm run dev` for a
live-reloading dev server while iterating on the UI.

Because it's a normal Vite project you can pull in any npm packages and any
framework (React, Vue, Svelte, plain JS, …) — just make sure the `"muxy"`
paths point at the files Vite emits into `dist/`.

## Use it

1. Copy this folder as your starting point.
2. Rename it and set `package.json`'s top-level `name` to the same name.
3. Edit the UI and iterate (`npm run dev`).
4. `npm run build` to produce the installable `dist/`.

See the [contributing guide](../../contributing.md) for the full
create → validate → publish flow, and the [extension docs](../../README.md)
for the complete `window.muxy` API.

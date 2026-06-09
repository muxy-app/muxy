# Muxy Starter — Vanilla JS + Tailwind

A minimal Muxy extension in plain JavaScript with [Tailwind CSS](https://tailwindcss.com) and Vite. It adds a pinned **Hello** panel, a topbar icon, and a palette command (`cmd+shift+h`) that toggles the panel. The panel header's refresh button fires `command.refresh-hello`, which the panel listens for.

```bash
npm install
npm run build
```

After rebuilding, click **Reload** in the Muxy Extensions modal to pick up changes (`npm run dev` runs Vite's dev server for fast iteration).

## Layout

- `panel/index.html` — panel entry, builds to `dist/`.
- `src/main.js` — mounts the panel onto `#root`.
- `src/panel/app.js` — the panel UI, rendered with the `h()` DOM helper.
- `src/lib/` — tiny `dom` and `icon` helpers.
- `src/styles/global.css` — Tailwind, with `--color-*` mapped to the app's `--muxy-*` theme tokens so utilities like `bg-primary` and `text-muted-foreground` follow the active theme.

See the [extension docs](https://github.com/muxy-app/muxy/tree/main/docs/extensions).

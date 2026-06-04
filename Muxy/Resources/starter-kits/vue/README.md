# Muxy Starter — Vue

A minimal Muxy extension in Vue 3 + Vite. It adds a pinned **Hello** panel, a topbar icon, and a palette command (`cmd+shift+h`) that toggles the panel. The panel header's refresh button fires `command.refresh-hello`, which the panel listens for.

```bash
npm install
npm run build
```

Copy the folder to `~/.config/muxy/extensions/muxy-starter-vue/` and rebuild to pick up changes. Theme colors use `var(--muxy-*)` and spacing follows a fixed scale; see the [extension docs](https://github.com/muxy-app/muxy/tree/main/docs/extensions).

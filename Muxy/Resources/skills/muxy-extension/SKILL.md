---
name: muxy-extension
description: Best-practice guide for authoring a Muxy extension — how it should look and behave so it reads as a native part of the app. Covers theming (follow the theme, never hardcode colors), the sizing scale, and which surface to use. Mechanics (manifest fields, permissions, the window.muxy API) live in the linked docs.
---

# Muxy Extension Guide

A Muxy extension is an npm + [Vite](https://vitejs.dev) project: source under `src/`, an entry HTML that `vite build` emits into `dist/`, and Muxy reads `dist/` when present (otherwise the project folder). The manifest is the `"muxy"` object in `package.json`. There is no fixed folder layout — every `entry`/`background`/icon path is an arbitrary relative path inside the build output (the vanilla starter kit emits its panel to `panel/index.html`); `package.json` and `dist/` are the only names Muxy fixes. During development you don't copy into the config folder — **Load Unpacked** in the Extensions modal points Muxy at any folder (your git checkout *is* the install).

**The `build` script must copy `package.json` into `dist/`.** The publish pipeline ships **only** `dist/`, and the app reads the manifest from the install root — so the manifest has to be inside the build output. `vite build` alone emits your entry/asset paths but **not** the manifest, so use `"build": "vite build && node scripts/copy-manifest.mjs"` where `copy-manifest.mjs` copies `package.json` into `dist/`. Easy to miss because **Load Unpacked** falls back to the root `package.json` in dev, so it loads locally but fails validation/install when published. The vanilla starter kit already wires this up.

**This skill is the guidance layer — how an extension should look and behave.** For the API and manifest mechanics (every field, the permission strings, the full `window.muxy` surface, events, scripts), read the reference docs. Start from the LLM-friendly index, which lists every page and links to its raw Markdown source:

> **<https://muxy.app/llms.txt>**

Append `/plain` to any docs URL for the raw Markdown of that page (e.g. <https://muxy.app/docs/extensions/manifest/plain>).

The goal of everything below: an extension should be indistinguishable from a native Muxy surface. Match the theme and match the scale, and it will be.

## Pick the right surface

- **Showing something to the user** → a **UI page** (tab, panel, or popover). Page scripts get the full `window.muxy` API.
- **A persistent, full-height navigation or control surface that replaces the built-in left sidebar** → a **`sidebar`** (one per extension; the user selects it in Settings → Sidebar). It fills the entire region — the project list *and* the footer — so own your own navigation. Same `window.muxy` API and theme variables as a panel.
- **Reacting durably to events, coordinating multiple webviews, or running shell commands headlessly** → a **`background.js`** script. It can also call `muxy.tabs.open` to show a result in the active workspace. Most extensions don't need one.
- **One-shot logic from the palette** → a **`runScript`** command, not a hidden tab. Its `muxy.*` calls are **synchronous** (return values directly — no `await`). It has `tabs`/`panes`/`projects`/`worktrees`/`browser`/`agents`/`files`/`git`/`exec`/`dialog`/`modal`/`topbar`/`statusbar`/`notifications`, but **not** `http`, `events`, `remote`, `panels`, or `popover`. It can open a modal and act on the choice inline — no page or background listener needed.

Don't open a hidden tab to run logic, and don't put durable event-driven work in tab JS where closing the tab loses it. Use `muxy.events.emit('extension.<name>', payload)` plus a background listener when a webview needs to ask background.js for shared or long-lived work.

## Theme — follow it, never hardcode

Muxy ships paired light/dark themes and a user-chosen accent. Every extension webview inherits CSS custom properties on `document.documentElement` that track the live theme and update automatically when the user switches it.

**Rules:**

1. **No hex literals for chrome.** Use `var(--muxy-…)` for every color. The only exception is decorative art meant to be theme-independent.
2. **The variables already invert** for light/dark — never sniff the color scheme to pick a color. Only branch on `muxy.theme.colorScheme` for things a variable can't express (e.g. swapping a logo image).
3. **`--muxy-accent` is the only saturated color.** Use it sparingly — primary action, focus ring, one key number — so it stays distinctive. Text *on* an accent fill should be `--muxy-background` to stay legible in both themes.
4. **Depth comes from `--muxy-surface` + `--muxy-border` + `--muxy-hover`,** not from new colors. Cards, inputs, code blocks, and buttons all share the one surface color.
5. **Re-read the theme for JS-drawn color.** Canvas/SVG that doesn't pick up CSS variables must redraw in `muxy.onThemeChange(theme => …)`.
6. **Popovers leave the body transparent** (`body { background: transparent; }`) — they sit over native macOS popover material that is already light/dark-aware. Tabs and panels *do* paint `--muxy-background` on the body.

**The variables (the complete injected set):**

| Variable | Use for |
| --- | --- |
| `--muxy-background` | Page background |
| `--muxy-foreground` | Primary text |
| `--muxy-foreground-muted` | Secondary text, labels, captions |
| `--muxy-surface` | Cards, inputs, code blocks, buttons |
| `--muxy-border` | 1px borders and dividers |
| `--muxy-hover` | Hover state for buttons / rows |
| `--muxy-accent` | Primary action, links, focus rings |
| `--muxy-accent-soft` | Translucent accent for badges/highlights |
| `--muxy-diff-add` / `--muxy-diff-remove` / `--muxy-diff-hunk` | Diff / success / error / hunk colors |
| `--muxy-topbar-height` | The app's tab-bar height (see Sizing) |

(`muxy.theme.colorScheme` gives `"light"`/`"dark"` in JS; there is no `--muxy-color-scheme` CSS var.)

## Sizing — match the app's scale

Muxy's native views are built from one scale of values, and **all of them scale with the user's interface-scale setting** (Settings → Interface). Pick from this scale rather than inventing numbers, so your surface tracks scale changes the way native views do. These are the base (100%) values in px:

**Spacing** (padding, `gap`, margin) — `2 · 4 · 6 · 8 · 10 · 12 · 16 · 20 · 24 · 32`. No in-between values. Panel rows and content pad `10px` left/right; an icon-and-label gap is `8px`; adjacent icon buttons sit `4px` apart.

**Font sizes** — `10` caption · `11` footnote/section labels (often uppercased) · **`12` body** (paths, row text) · `13` controls · **`14` titles** (weight 600) · `16`+ headings. Body is `12`, not `13`. Use the system font for UI; `"SF Mono", Menlo, monospace` for code, counts, and hashes.

**Icons** — `12`–`14px` glyphs at **weight 600** (a thinner default weight is the most common reason an extension's icons look foreign). Custom SVG strokes are `1.5px`, round caps/joins.

**Controls** — an icon button is a **`24×24` hit target** wrapping a `13`–`14px` glyph; text buttons are `28px` tall with `10px` horizontal padding.

**Radii** — `4` chips/badges · `6` buttons/inputs · `8` cards/panels · `10` large containers. Buttons are `4`–`6`, not `5`.

**Topbar height is the exception — never hardcode it.** It scales with interface scale and is injected pre-scaled as `--muxy-topbar-height`. A tab fills its whole region, so render your own topbar to match native tabs (so split panes line up): use that variable for the height and keep `box-sizing: content-box` so the 1px `border-bottom` lands on the same line as native tabs. Omit the topbar for edge-to-edge content.

Declare the scale once at the top of your stylesheet and reference it everywhere, so there are no stray magic numbers:

```css
:root {
  --s1:2px; --s2:4px; --s3:6px; --s4:8px; --s5:10px;
  --s6:12px; --s7:16px; --s8:20px; --s9:24px; --s10:32px;
  --font-caption:10px; --font-footnote:11px; --font-body:12px;
  --font-emphasis:13px; --font-title:14px;
  --icon-sm:12px; --icon:14px; --control:24px;
  --radius:6px; --radius-card:8px; --row-height:34px;
}
```

## Behavior

- **Least privilege.** Declare a permission only when you add the call that needs it.
- **Workspaces can be remote.** When the active workspace is a remote (SSH) workspace, `muxy.exec`, `muxy.git.*`, and worktree work run **on the remote server** with the selected SSH device's environment, and paths are remote paths. Write extensions against the active workspace, not a hardcoded local machine — the same code works for local and remote because Muxy brokers the SSH connection. See [Scripts](https://muxy.app/docs/extensions/scripts).
- **Use `muxy.git` for repository work** (status, diff incl. `{ raw: true }`, repoInfo, log, branches, PRs incl. `pr.number`/`pr.diff`, tags, init, checkout/cherryPick/revert, branch `delete`/`deleteRemote`, worktrees incl. `worktree.switchTo` and `pr.checkoutWorktree`) instead of shelling out via `muxy.exec` — it's the app's own git core, returns structured data, and caches reads. Reads need `git:read`; writes need `git:write` and prompt for consent. Reads are cached per project/worktree (HEAD/index aware); pass `{ fresh: true }` to bypass. Available to tabs, panels, popovers, `runScript` commands, and background scripts. See [Git](https://muxy.app/docs/extensions/git).
- **Manage the project list with `muxy.projects` write verbs** instead of editing `projects.json` — `add(path)` registers an existing folder as a project, makes it the active project, and returns its project id, `rename(identifier, name)`, `setColor`/`setIcon(identifier, value)`, `setLogo(identifier, storedLogoFilename)` (pass `null` to clear), and `reorder(identifiers)` (all local non-home project ids in the new order, each exactly once). All need `projects:write` and mutate Muxy's live project store, so the native sidebar updates immediately. `add` only accepts an existing directory; create the folder first via `muxy.files`/`muxy.exec` if needed. Subscribe to the `projects.changed` event (declare `events: ["projects.changed"]`, grant `projects:read`) to notify a webview/sidebar that can refetch via `muxy.projects.list()` after any change, whether made by your extension or Muxy itself. The home project cannot be renamed, recolored, re-iconed, or reordered. See [Permissions](https://muxy.app/docs/extensions/permissions).
- **`muxy.projects.delete(identifier)` deletes a project** and is irreversible — it cleans up the project's worktrees, branches, and directories on disk. It needs the `projects:delete` permission (separate from `projects:write`) and prompts the user for confirmation on every call. The home project cannot be deleted. Reserve it for explicit user-driven actions, never silent cleanup. See [Permissions](https://muxy.app/docs/extensions/permissions).
- **Use `muxy.files` for workspace filesystem work** (list, read, stat, write, mkdir, rename, move, delete) instead of `muxy.exec` — paths are sandboxed to the active worktree root and returned relative to it. Reads need `files:read`; writes need `files:write` and prompt for consent. Pair with the `file.changed` event to stay reactive (e.g. a file tree). See [Files](https://muxy.app/docs/extensions/files).
- **Persist your own state with `muxy.storage`** instead of shelling out to a config file — `set(key, value)` / `get(key)` (any JSON value; `get` returns `null` when absent) / `delete(key)` / `keys()`. Storage is isolated per extension and shared across that extension's surfaces (a panel and its `background.js` see the same keys), and survives restarts. Needs `storage:read` (`get`/`keys`) / `storage:write` (`set`/`delete`); a key is ≤256 chars, a value ≤1 MB. Good for layout/collapse/preferences. See [Storage](https://muxy.app/docs/extensions/storage).
- **Ask for a value or a folder with `muxy.dialog.prompt` / `muxy.dialog.pickFolder`** instead of `osascript` — `prompt({ title, message, default?, placeholder?, confirm?, cancel? })` resolves the entered string (or `null`), `pickFolder({ title?, message?, default? })` resolves an absolute path (or `null`). Same surfaces and no-permission rule as `confirm`/`alert`. See [Dialogs](https://muxy.app/docs/extensions/dialogs).
- **React to branch changes with the `worktree.headChanged` event** instead of polling `git.worktrees()` — it fires when a worktree's checked-out branch changes (e.g. a `git checkout` in a terminal), with the new `branch` and worktree `path`. Declare `events: ["worktree.headChanged"]` and grant `worktrees:read`. See [Events](https://muxy.app/docs/extensions/events).
- **React to AI agent activity with the `agent.status` event** instead of polling — it reports an agent's lifecycle per worktree (`working` > `waiting` > `idle`, with `providerID` and the owning `paneID`), driven by the provider's hooks (Claude Code, Cursor, Codex, Droid, Grok, OpenCode, Pi). Coverage depends on what each CLI's hooks expose — some report all of `working`/`waiting`/`idle`, others only a subset (e.g. Codex reports only `idle`, Cursor never reports `working`, Pi never reports `waiting`). It fires only when a worktree's status changes and turns `idle` when the last agent pane closes. Declare `events: ["agent.status"]` and grant `agents:read` (both the event subscription and `muxy.agents.list()` need it); pair the event with `muxy.agents.list()` to hydrate current statuses on load. Good for a live per-worktree indicator. See [Events](https://muxy.app/docs/extensions/events).
- **Use `muxy.http.fetch` to call external APIs from a tab/panel/popover** instead of the webview's `fetch()` — the request goes out via native code, so it is **not CORS-blocked**, and a panel needs no `background.js` (no subprocess) just to reach the network. Pass `(url, { method?, headers?, body?, timeoutMs? })` and `await` `{ status, headers, body, truncated }`. No manifest permission; the first call to a host prompts for consent, "Allow & remember" whitelists that host. Private/loopback hosts (localhost, `127.*`, `192.168.*`, `169.254.*`, `.local`, …) are blocked. `muxy.http` is a webview-only surface — neither background scripts nor `runScript` commands have `fetch`; they shell out via `muxy.exec(['curl', …])`. See [HTTP](https://muxy.app/docs/extensions/http).
- **Use `muxy.modal.open` for a list picker** (the native searchable picker overlay) instead of building your own — pass `{ items: [{ id, title, subtitle? }], placeholder?, onSelect(choice) }`; the choice (or `null` if dismissed) arrives in `onSelect`. Muxy owns the search, navigation, and open/close. No permission needed. Available on every surface: on `runScript`/background `modal.open` returns immediately and you read the result in `onSelect`; on webview pages you can also `await` it. It has no shortcut of its own: bind a palette `command` with a `defaultShortcut` (its action can be the `runScript` that opens the modal, or an `event` a `background.js` listener reacts to). Pass `searchToolbar: true` only when the picker should show the footer search option toggles (`Aa`, `W`, `.*`). **For large lists (a file picker over a big repo), pass `items` as a function `items(emit)` instead of an array** — the picker opens instantly and you stream rows with `emit(batch)` while Muxy filters them natively, so typing never calls back into your code and the UI can't hang. **For results that depend on the query (server-side/async search), pass `onQuery(query, emit)`** — Muxy debounces the field and calls it per query so you supply a fresh list, dropping responses for superseded queries; native filtering still runs on top. See [Modal](https://muxy.app/docs/extensions/modal).
- **Bind keyboard shortcuts to your extension** — for a *static* binding declare a palette `command` with a `defaultShortcut` (`"cmd+shift+e"`); for a *runtime* one (e.g. configurable in your settings) call `muxy.shortcuts.register({ id, combo })` from `background.js` and subscribe to the same `command.<id>` event. `register` returns `{ ok, conflict? }`; `unregister(id)` and `list()` round it out. Runtime shortcuts need `shortcuts:register`, are not persisted (re-register on launch), and reject an `id` that collides with a manifest command. See [Palette Commands](https://muxy.app/docs/extensions/palette-commands).
- **Build your own session restore from `background.js`** — Muxy has no built-in session restore, so an extension owns it. Record sessions by subscribing to the enriched `tab.*` events (which now carry `kind`/`projectID`/`worktreeID`/`areaID`/`cwd`/`data`), then recreate each terminal with `muxy.tabs.open({ kind: 'terminal', directory, command })`, which resolves the new tab's id (for `extensionWebView` tabs, the instance id usable with `setTitle`/`setIcon`). `directory` stays inside the worktree root; `command` adds a one-time runtime consent on top of `tabs:write`. See [Events](https://muxy.app/docs/extensions/events) and [Tabs](https://muxy.app/docs/extensions/tabs).
- **Use `extension.*` events for webview ↔ background communication** — pages and background scripts can `muxy.events.subscribe('extension.<name>', handler)` and `muxy.events.emit('extension.<name>', payload)`. These events are same-extension only, need no permission, and are not listed in the manifest `events` array. A webview emit is relayed through the extension's `background.js`, so it rejects when no background script is running — webviews can't reach each other directly. Workspace events (`pane.*`, `file.changed`, etc.) still require manifest `events`.
- **Update bar items live with `muxy.topbar.set` / `muxy.statusbar.set`** — pass `{ id, icon?, visible? }` (topbar) or `{ id, icon?, text?, visible? }` (statusbar) from `background.js` or any page to swap the icon/text or show/hide without reloading; `text: null` clears back to the manifest value. Decide visibility at runtime: declare the item with `"visible": false` and call `muxy.topbar.show(id)` / `.hide(id)` (or `muxy.statusbar.show(id)` / `.hide(id)`) when it applies. The item must be declared in `topbarItems` / `statusBarItems`; needs `panels:write`. Good for live indicators (e.g. a PR badge that only appears inside a repo). See [Topbar](https://muxy.app/docs/extensions/topbar) / [Status bar](https://muxy.app/docs/extensions/statusbar).
- **Retitle a tab live with `muxy.tabs.setTitle(title)` / `muxy.tabs.setIcon(icon)`** from the tab's own page to reflect changing state (e.g. an editor showing the open file). `icon` is `"<sf-symbol>"`, `{ symbol }`, or `{ svg }`; `setTitle("")` / `setIcon(null)` reset to the manifest defaults. Needs `tabs:write`; runtime-only (resets on restart, so set it again on load). See [Tabs](https://muxy.app/docs/extensions/tabs).
- **Autofocus your input on `muxy.onFocus`** from a tab/panel/popover page so the surface behaves like a native one — when its tab is opened or switched back to, move keyboard focus into your editor or search field. The callback receives `true` on focus gained, `false` on lost; `muxy.focused` reads the current state. No permission, no manifest field. See [Tabs](https://muxy.app/docs/extensions/tabs).
- **Guard a close with `muxy.lifecycle.onBeforeClose(handler)`** from a tab/panel/popover page when closing could lose work (a dirty editor). Return/resolve `true` (or `{ prevent: true }`) to prevent the close, anything else to allow it; the handler may be `async`, so `await muxy.dialog.confirm(...)` and decide. Call `muxy.lifecycle.close()` to finish the close yourself without re-asking. No permission, no manifest field — registering the handler is the opt-in, and it fails open (no handler / timeout / throw ⇒ closes). It does **not** fire on app quit or an outside-click popover dismiss; for those, persist reactively instead. To merely react after a close, subscribe to `tab.closed` / `panel.closed` / `popover.closed`. See [Lifecycle](https://muxy.app/docs/extensions/lifecycle).
- **Drive and automate the built-in browser with `muxy.browser.*`.** Tabs: `open(url, { split })` returns the tab ID; `navigate(tabId, url)`, `reload/back/forward(tabId)`, `list()`, `read(tabId)` (`{ title, url, text }`, ~1 MB cap), `close(tabId)`. Automation: `eval(tabId, script)` (returns the parsed JS result), `click`, `type(…, { submit })`, `fill`, `press(tabId, key, selector?)`, `select`, `hover`, `scrollIntoView`, `setChecked`. Waiting: `wait(tabId, { selector|text|urlContains|function, timeoutMs })`, `waitFor`, `waitForNavigation`. Inspection: `getText/getHTML/getValue/getAttribute/getCount`, `is(tabId, property, selector)`, `find(tabId, kind, value)`, `snapshot(tabId, selector?)` (visible interactive elements — let an agent "see" the page), `screenshot(tabId)` (base64 PNG). State: `storage.get/set/clear(tabId, key, value?, kind)` for local/session storage; `cookies.get/set/delete/clear(tabId, ...)` per profile. Reads and JS-running calls (`eval`, `click`, `type`, `waitFor`, `get*`, `screenshot`, `storage.*`) need `browser:read`/`browser:write` and require the tab open and rendered in the active project — there is no headless browser; `navigate`, `cookies`, `list` do not. Every call fails when the user disables the built-in browser. Capture the tab ID from `open`/`list` and reuse it. See [Browser](https://muxy.app/docs/extensions/browser).
- **Make hover and active states visible** in both light and dark — `background: var(--muxy-hover); border-color: var(--muxy-accent);` is the standard pattern.
- **Respect `prefers-reduced-motion`** — Muxy users opt into Reduce Motion at the OS level; avoid long transitions, large translations, autoplay.
- **No hardcoded `~/.config/muxy` paths** from inside the extension — rely on the working directory Muxy sets, or pass `cwd` to `exec`.

## Checklist

- [ ] Every color is `var(--muxy-…)`; `muxy.onThemeChange` wired for any JS-drawn color.
- [ ] Spacing, font, icon, control, and radius values come from the scale above — no off-ramp numbers (rows pad `10px`, body is `12px`, icons `12`–`14px` at weight 600).
- [ ] Tab topbar uses `--muxy-topbar-height` with `box-sizing: content-box`.
- [ ] Hover/active states are visible in both themes.
- [ ] `permissions` declares only what is used.
- [ ] Durable event-driven work is in `background.js`, not tab JS. Webview coordination uses `extension.*` events. No background script unless events, shared state, or background `exec` are needed.
- [ ] `build` copies `package.json` into `dist/` (e.g. `vite build && node scripts/copy-manifest.mjs`) — only `dist/` ships, so the manifest must be inside it.
- [ ] Built with `npm run build`, then **Reload** in the Extensions modal (a Reload alone won't pick up unbuilt source).

# Browser

The desktop app has a built-in browser. A browser is a pane like a terminal,
so it opens in a tab or a split, and its page keeps running while hidden.

## Open a page

- **File → New Browser Tab** (`Cmd+Opt+B`), the globe button in the tab
  strip, or the command palette.
- `muxy browser open <url>` from a terminal. See [Muxy CLI](muxy-cli.md#browser-control).
- Command-click a link in a terminal, when **Settings → Browser → Open terminal
  links in the browser** is on.
- A page that opens a new window gets a new tab instead.

Type an address or a search in the address bar. Matching history shows below
it. Links to other apps, such as `mailto:`, open in those apps.

## In a page

| Action | Shortcut |
| --- | --- |
| Address bar | `Cmd+L` |
| Reload | `Cmd+R` |
| Back / forward | `Cmd+[` / `Cmd+]` |
| Find on page | `Cmd+F` |
| Zoom in / out / reset | `Cmd+=` / `Cmd+-` / `Cmd+0` |
| Copy the page's address | `Cmd+Shift+C` |
| Web Inspector | `Cmd+Opt+I` |

The toolbar also opens the page in another browser and switches the tab's
profile.

## Profiles

Each profile keeps its own cookies, site data, history, and logins. New tabs use
the default profile. Manage profiles in **Settings → Browser**, where you can
also import history, bookmarks, and cookies from another browser on this
computer.

## Saved and not saved

A browser tab remembers its address and profile across restarts. Its title,
zoom, and back/forward history start fresh.

## Turning it off

Turn off **Settings → Browser → Enable built-in browser** to disable browser
tabs and `muxy browser`. Terminal links then open in your system browser.

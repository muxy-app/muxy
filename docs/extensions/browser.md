# Browser

`muxy.browser` lets extensions open and fully automate Muxy's built-in browser tabs — navigation, DOM interaction, JavaScript, cookies, storage, and screenshots.

```js
const tabId = await muxy.browser.open("https://example.com", { split: true });
await muxy.browser.waitFor(tabId, "input[name=q]");
await muxy.browser.type(tabId, "input[name=q]", "muxy", { submit: true });
await muxy.browser.waitForNavigation(tabId);
const title = await muxy.browser.eval(tabId, "document.title");
const png = await muxy.browser.screenshot(tabId);
```

## Tabs

`open(url?, options?)` returns the new browser tab ID. Omit `url` for the configured home page. Pass `{ split: true }` to open beside the current pane.

`navigate(tabId, url)` loads a new URL. `reload(tabId)`, `back(tabId)`, `forward(tabId)` drive history.

`list()` returns `{ id, title, url, profile, isActive }` for browser tabs. `close(tabId)` closes a tab.

`read(tabId)` returns `{ title, url, text }` from the rendered page (text capped near 1 MB).

## Automation

`eval(tabId, script)` runs JavaScript in the page and returns the parsed result. A single-line expression (e.g. `document.title`) is returned directly; a multi-statement script (containing `;` or newlines) runs as a function body and must `return` its result. Scripts may `await`.

`click(tabId, selector)` clicks the first matching element; returns `true` if found.

`type(tabId, selector, text, options?)` focuses an element, sets its value, fires `input`/`change`. Pass `{ submit: true }` to submit the form.

`waitFor(tabId, selector, options?)` polls until the selector exists or `options.timeoutMs` (default 5000) elapses; returns whether it appeared.

`waitForNavigation(tabId, options?)` resolves when the page finishes loading or `options.timeoutMs` (default 10000) elapses; returns the settled URL.

`getText(tabId, selector)`, `getHTML(tabId, selector?)`, `getAttribute(tabId, selector, name)` read the DOM. `getHTML` with no selector returns the full document.

`screenshot(tabId)` returns a base64-encoded PNG of the rendered page.

## Storage & Cookies

```js
await muxy.browser.storage.set(tabId, "token", "abc", "local");
const token = await muxy.browser.storage.get(tabId, "token", "local");

const cookies = await muxy.browser.cookies.get(tabId);
await muxy.browser.cookies.set(tabId, { name: "session", value: "x", domain: "example.com" });
await muxy.browser.cookies.delete(tabId, "session");
```

`storage.get/set/clear(tabId, ...)` access `localStorage` (default) or `sessionStorage` (pass `"session"`).

`cookies.get/set/delete/clear` operate on the tab's profile. Cookies are shared by every tab using the same profile, so `cookies.clear` affects them all.

## Requirements

Automation that runs JavaScript (`eval`, `click`, `type`, `waitFor`, `get*`, `screenshot`, `storage.*`) requires the tab to be open and rendered in the active project — Muxy has no headless browser. `navigate`, `cookies.*`, and `list` do not require the tab to be visible.

## Permissions

Declare `browser:read` for `list`, `read`, `waitFor`, `get*`, `screenshot`, `waitForNavigation`, `storage.get`, and `cookies.get`. Declare `browser:write` for `open`, `navigate`, `close`, `eval`, `click`, `type`, `reload`, `back`, `forward`, `storage.set/clear`, and `cookies.set/delete/clear`.

All browser calls fail if the user disables the built-in browser in Settings. These APIs are available to extension webviews and background scripts.

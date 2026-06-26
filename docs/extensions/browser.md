# Browser

`muxy.browser` lets extensions open and drive Muxy's built-in browser tabs.

```js
const tabId = await muxy.browser.open("https://example.com", { split: true });
await muxy.browser.navigate(tabId, "https://example.com/docs");
const tabs = await muxy.browser.list();
const page = await muxy.browser.read(tabId);
await muxy.browser.close(tabId);
```

`open(url?, options?)` returns the new browser tab ID. Omit `url` to use the configured home page. Pass `{ split: true }` to open beside the current pane.

`navigate(tabId, url)` loads a new URL in an existing browser tab.

`list()` returns `{ id, title, url, profile, isActive }` for browser tabs.

`read(tabId)` returns `{ title, url, text }` from the rendered page. Text is capped at about 1 MB, and the tab must be rendered in the active project.

`close(tabId)` closes the browser tab.

## Permissions

Declare `browser:read` for `list()` and `read()`. Declare `browser:write` for `open()`, `navigate()`, and `close()`.

All browser calls fail if the user disables the built-in browser in Settings.

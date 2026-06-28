# HTTP

`muxy.http` lets an extension's **tab, panel, or popover** call external HTTP(S) APIs from native code (`URLSession`) instead of the webview. Because the request leaves from Muxy rather than the `muxy-ext://` page origin, it is **not subject to CORS** — the usual blocker when an extension page tries to `fetch()` a third-party API directly. It also means a panel that only needs to call an API does **not** have to declare a background script, so no host subprocess is spawned.

The single method is async (returns a `Promise`).

## `muxy.http.fetch(url, options?)`

```js
const res = await muxy.http.fetch("https://api.github.com/repos/muxy-app/muxy", {
  method: "GET",
  headers: { Accept: "application/vnd.github+json" },
});
// { status, headers, body, truncated }
const repo = JSON.parse(res.body);
```

`options`:

| Field | Default | Notes |
| --- | --- | --- |
| `method` | `"GET"` | One of `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`. |
| `headers` | — | Object of string header values. `Host`, `Content-Length`, `Connection` are ignored. |
| `body` | — | Request body string (e.g. a JSON string). |
| `timeoutMs` | `30000` | Request timeout in milliseconds. |

The result is `{ status, headers, body, truncated }`. `body` is the response text; responses over 10 MB are truncated and `truncated` is `true`.

## No manifest permission — host consent instead

`muxy.http` is **not** gated by a manifest permission. The first request to a given host prompts the user for [runtime consent](permissions.md#runtime-consent), showing the host, method, and URL:

- **Allow & remember** — runs the call and whitelists that **host** for this extension; future requests to the same host never prompt.
- **Allow** — runs this one call, asks again next time.
- **Cancel** / **Deny & remember** — denies the call (and optionally writes a deny rule for the host).

Consent is keyed by host, so allowing `api.github.com` does not allow `example.com`.

## Blocked hosts

Requests to private and loopback addresses are **rejected before any prompt**, to prevent an extension from reaching internal services (SSRF):

- `localhost`, `*.localhost`, `*.local`
- `127.0.0.0/8`, `10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`, `169.254.0.0/16`
- IPv6 `::1`, `fc00::/7`, `fe80::/10`

Hostnames are resolved before the check, so an alias that points at a private address is blocked too. A host that fails to resolve is blocked. Redirects are re-validated against the same policy.

## Errors

A rejected promise carries a message string:

- `http: blocked request to private or loopback host '…'` — SSRF guard rejected the host.
- `http: only http and https URLs are allowed` — unsupported scheme.
- `http: unsupported method '…'` — method not in the allowed set.
- `http request failed: user denied consent for …` — the consent prompt was denied.
- Anything else surfaces the underlying networking error text.

## Notes

- `muxy.http` is available only to extension **tabs**, **panels**, and **popovers** (the WKWebView surfaces). Neither background scripts nor [`runScript`](scripts.md) commands have it (JavaScriptCore has no `fetch`); they can still shell out via `muxy.exec(['curl', …])`.
- Use this instead of the webview's `fetch()` whenever the target API is cross-origin and does not send permissive CORS headers.

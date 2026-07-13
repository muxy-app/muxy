# GitHub

`muxy.gh` exposes the authenticated GitHub CLI account to extensions without shelling out to `gh api`. It reads the identity of whoever is signed in to the [`gh` CLI](https://cli.github.com) on the machine — the same account Muxy's own git integration uses.

On tabs/panels/popovers these methods return a `Promise` (use `await`); in [`runScript`](scripts.md) commands and background scripts the same call is **synchronous** and returns the value directly. The account is global to the `gh` login, so no `{ project }` argument is needed.

## Permissions

| Permission | Methods |
| --- | --- |
| `gh:read` | `user` |

```json
{
  "name": "gh-badge",
  "version": "0.1.0",
  "permissions": ["gh:read"]
}
```

## `muxy.gh.user()`

Returns the signed-in GitHub user, or rejects if the `gh` CLI is not installed or not authenticated.

```js
const me = await muxy.gh.user();
// { login: "octocat", name: "The Octocat", avatarUrl: "https://…" }
```

| Field | Type | Notes |
| --- | --- | --- |
| `login` | `string` | GitHub username |
| `name` | `string` | Display name; empty string if the account has none |
| `avatarUrl` | `string` | Avatar image URL; empty string if unavailable |

The result is cached in memory for five minutes, so a badge or header can call it on every render without spawning a `gh` process each time. It runs on the **local machine** — it reflects the local `gh` login even when the active workspace is a remote (SSH) one.

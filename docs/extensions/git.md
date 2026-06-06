# Git

`muxy.git` gives extensions full programmatic access to the repository behind the active project — status, diffs, history, branches, pull requests, and worktrees. It is the same git core the app and the mobile remote use, so there is one source of truth for everything git.

On tabs/panels/popovers these methods return a `Promise` (use `await`); in [`runScript`](scripts.md) commands and background scripts the same calls are **synchronous** and return the value directly. They operate on the **active worktree of a project**. Pass `{ project }` (a project id, name, or path) to target a specific project; omit it to use the active one.

> Use `muxy.git` instead of shelling out with `muxy.exec` — it returns structured data, is cached, and avoids spawning a `git`/`gh` process per call.

## Caching

Read methods are cached per project/worktree. A cached value is reused only while the repository's `HEAD` and index are unchanged, so a commit, stage, or checkout — from the extension **or** an external terminal — invalidates it automatically. There is also a short time bound so network-backed `pr.*` calls don't run on every request.

Pass `{ fresh: true }` to any read method to bypass the cache and force a refresh:

```js
await muxy.git.status();              // cached
await muxy.git.status({ fresh: true }); // always recomputed
```

## Permissions

| Permission | Methods |
| --- | --- |
| `git:read` | `status`, `diff`, `repoInfo`, `log`, `branches`, `remoteBranches`, `currentBranch`, `aheadBehind`, `pr.info`, `pr.number`, `pr.diff`, `pr.list`, `worktrees` |
| `git:write` | `init`, `stage`, `unstage`, `discard`, `commit`, `push`, `pull`, `checkout`, `cherryPick`, `revert`, `branch.create`, `branch.switchTo`, `branch.delete`, `branch.deleteRemote`, `tag.create`, `pr.create`, `pr.merge`, `pr.close`, `pr.checkout`, `pr.checkoutWorktree`, `worktree.add`, `worktree.remove`, `worktree.switchTo` |

Every **write** also prompts the user for [runtime consent](permissions.md#runtime-consent) the first time, remembered as an allow/deny rule for the extension.

```json
{
  "name": "git-tools",
  "version": "0.1.0",
  "permissions": ["git:read", "git:write"]
}
```

## Read methods

### `muxy.git.status(opts?)`

Returns a snapshot of the working tree:

```js
const s = await muxy.git.status();
// {
//   branch: "feature/x",
//   aheadBehind: { ahead: 2, behind: 0, hasUpstream: true },
//   defaultBranch: "main",
//   branches: ["main", "feature/x"],
//   stagedFiles:   [{ path, oldPath, status, isStaged, isUnstaged, isBinary, additions, deletions }],
//   unstagedFiles: [ ... ],
//   pullRequest: null | { url, number, state, isDraft, baseBranch, mergeable, mergeStateStatus, isCrossRepository, checks }
// }
```

`status` on each file is the git status letter (`M`, `A`, `D`, `R`, `?`, …).

Pass `{ local: true }` to skip pull-request enrichment — it avoids the `gh` network call and returns `pullRequest: null`, which is faster when you only need the working tree.

### `muxy.git.repoInfo(opts?)`

```js
await muxy.git.repoInfo();
// { root, gitDir, isWorktree, currentBranch }
```

Resolves the repository layout in a single call — `root` (top level), `gitDir`, whether the active checkout is a linked `isWorktree`, and the `currentBranch`.

### `muxy.git.diff(opts)`

```js
const d = await muxy.git.diff({ filePath: "src/main.swift", staged: false });
// { additions, deletions, truncated, rows: [{ kind, oldLineNumber, newLineNumber, oldText, newText, text }] }

const raw = await muxy.git.diff({ raw: true });
// { diff: "<unified diff text>", truncated }
```

- `filePath` — path relative to the repo root. Required for the parsed mode; optional in `raw` mode (omit for the whole repo).
- `raw` — `true` returns the raw unified diff string instead of parsed rows.
- `staged` — `true` for the staged diff, `false`/omitted for the working-tree diff.
- `lineLimit` — cap the number of lines (omit for full).

`kind` is one of `hunk`, `context`, `addition`, `deletion`, `collapsed`.

### `muxy.git.log(opts?)`

```js
const commits = await muxy.git.log({ maxCount: 50, skip: 0 });
// [{ hash, shortHash, subject, authorName, authorDate, isMerge, parentHashes, refs: [{ name, kind }] }]
```

### `muxy.git.branches(opts?)` · `muxy.git.remoteBranches(opts?)` · `muxy.git.currentBranch(opts?)` · `muxy.git.aheadBehind(opts?)`

```js
await muxy.git.branches();        // ["main", "feature/x"]
await muxy.git.remoteBranches();  // ["origin/main", "origin/feature/x"]
await muxy.git.currentBranch();   // "feature/x"
await muxy.git.aheadBehind();     // { ahead, behind, hasUpstream }
```

### `muxy.git.pr.info(opts?)` · `muxy.git.pr.number(opts?)` · `muxy.git.pr.diff(opts)` · `muxy.git.pr.list(opts?)`

```js
await muxy.git.pr.info();                                  // PR for the current branch, or null
await muxy.git.pr.number();                                // just the PR number for the current branch, or null
await muxy.git.pr.diff({ number: 42 });                    // { diff: "<unified diff text>", truncated }
await muxy.git.pr.list({ filter: "open", limit: 50 });     // filter: open | closed | merged | all
```

`pr.number` is a cheap way to learn whether the current branch has a PR without fetching its full status. `pr.diff` fetches the PR head and returns the raw diff against its merge base. All require the GitHub CLI (`gh`) to be installed and authenticated.

### `muxy.git.worktrees(opts?)`

```js
await muxy.git.worktrees();
// [{ path, branch, head, isBare, isDetached, isPrunable }]
```

## Write methods

All writes prompt for consent on first use.

```js
await muxy.git.init();                              // git init in the active project folder

await muxy.git.stage({ paths: ["a.txt"] });        // empty paths => stage all
await muxy.git.unstage({ paths: ["a.txt"] });      // empty paths => unstage all
await muxy.git.discard({ paths: [], untrackedPaths: ["tmp.log"] });

await muxy.git.commit({ message: "Fix bug", stageAll: true }); // => { hash }
await muxy.git.push();                      // sets upstream automatically if missing
await muxy.git.push({ setUpstream: true }); // always push --set-upstream origin <branch>
await muxy.git.pull();

await muxy.git.checkout({ hash: "a1b2c3d" });          // detached checkout of a commit
await muxy.git.cherryPick({ hash: "a1b2c3d" });
await muxy.git.revert({ hash: "a1b2c3d" });            // staged, not committed

await muxy.git.branch.create({ name: "feature/y" });   // creates and switches
await muxy.git.branch.switchTo({ branch: "main" });
await muxy.git.branch.delete({ name: "feature/old", force: false }); // local delete (-d, or -D when force)
await muxy.git.branch.deleteRemote({ branch: "feature/old" });

await muxy.git.tag.create({ name: "v1.0.0", hash: "a1b2c3d" });

await muxy.git.pr.create({ title: "Add Y", body: "…", baseBranch: "main", draft: false }); // => PR info
await muxy.git.pr.merge({ number: 42, method: "squash", deleteBranch: true }); // method: merge | squash | rebase
await muxy.git.pr.close({ number: 42 });
await muxy.git.pr.checkout({ number: 42 });                              // checks the PR out locally
await muxy.git.pr.checkoutWorktree({ number: 42, path: "~/code/pr-42" }); // => { branch }

await muxy.git.worktree.add({ path: "~/code/app-y", branch: "feature/y", createBranch: true, baseBranch: "main" });
await muxy.git.worktree.remove({ path: "~/code/app-y", force: false });
await muxy.git.worktree.switchTo({ identifier: "feature/y" }); // activate a worktree (id, name, branch, or path)
```

## Errors

A rejected promise carries a message string:

- `permission denied (git:read|git:write)` — missing manifest permission.
- `user denied consent for git.<op>` — the write consent prompt was denied.
- `project not found …` — the `project` selector did not resolve.
- `invalid arguments …` — a required field was missing.
- Anything else surfaces the underlying git/`gh` error text.

```js
try {
  await muxy.git.commit({ message: "" });
} catch (err) {
  console.error(err.message); // "commit message is required"
}
```

## Notes

- `muxy.git` is available to extension **tabs**, **panels**, **popovers**, **`runScript` commands**, and **background scripts** — the same API and permissions everywhere. Calls return a `Promise` on webview pages and are synchronous in `runScript` and background scripts.
- The app continues to own the worktree lifecycle it shows in the sidebar; `git.worktree.*` operates on the same underlying git worktrees, so changes are reflected after a refresh.
- `worktree.remove` runs the app's full teardown (hooks, branch, directory) and updates the sidebar. With `force: false` it rejects a worktree that has uncommitted changes; pass `force: true` to discard them.
- There are no AI helpers here — generate commit messages or PR bodies with your own model via `muxy.exec` if you need them.

# Git

`muxy.git` gives extensions full programmatic access to the repository behind the active project — status, diffs, history, branches, pull requests, and worktrees. It is the same git core the app and the mobile remote use, so there is one source of truth for everything git.

All methods are async (return a `Promise`) and operate on the **active worktree of a project**. Pass `{ project }` (a project id, name, or path) to target a specific project; omit it to use the active one.

## Permissions

| Permission | Methods |
| --- | --- |
| `git:read` | `status`, `diff`, `log`, `branches`, `remoteBranches`, `currentBranch`, `aheadBehind`, `pr.info`, `pr.list`, `worktrees` |
| `git:write` | `stage`, `unstage`, `discard`, `commit`, `push`, `pull`, `checkout`, `cherryPick`, `revert`, `branch.create`, `branch.switchTo`, `branch.deleteRemote`, `tag.create`, `pr.create`, `pr.merge`, `pr.close`, `pr.checkout`, `pr.checkoutWorktree`, `worktree.add`, `worktree.remove`, `worktree.switchTo` |

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

### `muxy.git.diff(opts)`

```js
const d = await muxy.git.diff({ filePath: "src/main.swift", staged: false });
// { additions, deletions, truncated, rows: [{ kind, oldLineNumber, newLineNumber, oldText, newText, text }] }
```

- `filePath` (required) — path relative to the repo root.
- `staged` — `true` for the staged diff, `false`/omitted for the working-tree diff.
- `lineLimit` — cap the number of parsed lines (omit for full).

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

### `muxy.git.pr.info(opts?)` · `muxy.git.pr.list(opts?)`

```js
await muxy.git.pr.info();                                  // PR for the current branch, or null
await muxy.git.pr.list({ filter: "open", limit: 50 });     // filter: open | closed | merged | all
```

Both require the GitHub CLI (`gh`) to be installed and authenticated.

### `muxy.git.worktrees(opts?)`

```js
await muxy.git.worktrees();
// [{ path, branch, head, isBare, isDetached, isPrunable }]
```

## Write methods

All writes prompt for consent on first use.

```js
await muxy.git.stage({ paths: ["a.txt"] });        // empty paths => stage all
await muxy.git.unstage({ paths: ["a.txt"] });      // empty paths => unstage all
await muxy.git.discard({ paths: [], untrackedPaths: ["tmp.log"] });

await muxy.git.commit({ message: "Fix bug", stageAll: true }); // => { hash }
await muxy.git.push();   // sets upstream automatically if missing
await muxy.git.pull();

await muxy.git.checkout({ hash: "a1b2c3d" });          // detached checkout of a commit
await muxy.git.cherryPick({ hash: "a1b2c3d" });
await muxy.git.revert({ hash: "a1b2c3d" });            // staged, not committed

await muxy.git.branch.create({ name: "feature/y" });   // creates and switches
await muxy.git.branch.switchTo({ branch: "main" });
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

- `muxy.git` is available to extension **tabs**, **panels**, and **popovers**. Background scripts only expose `exec`, `notifications.notify`, and `dialog.*`.
- The app continues to own the worktree lifecycle it shows in the sidebar; `git.worktree.*` operates on the same underlying git worktrees, so changes are reflected after a refresh.
- There are no AI helpers here — generate commit messages or PR bodies with your own model via `muxy.exec` if you need them.

# Git

`muxy.git` reads and changes the project's repository. It runs on the project's
server, so it works for remote projects too, and is available everywhere.

Calls take an optional options object. Pass `project` in it to target a
project or worktree by ID; otherwise a tab uses its own project and other views
use the current one.

```js
const status = await muxy.git.status();
await muxy.git.stage({ paths: ["src/main.rs"] });
await muxy.git.commit({ message: "Fix the parser" });
await muxy.git.push();
```

Reads need `git:read`. Writes need `git:write` and ask the user first, except
switching worktrees.

## Reading

| Call | Options | Returns |
| --- | --- | --- |
| `status` | `local` | `{ branch, aheadBehind, defaultBranch, branches, stagedFiles, unstagedFiles, pullRequest }` |
| `diff` | `filePath`, `staged`, `raw`, `lineLimit` | `{ additions, deletions, truncated, rows }`, or `{ diff, truncated }` with `raw` |
| `log` | `maxCount` (100), `skip` | `[{ hash, shortHash, subject, authorName, authorDate, isMerge, parentHashes, refs }]` |
| `repoInfo` | | `{ root, gitDir, isWorktree, currentBranch }` |
| `currentBranch` | | Branch name, or `null` |
| `aheadBehind` | | `{ ahead, behind, hasUpstream }` |
| `branches`, `remoteBranches` | | Branch names |
| `worktrees` | | `[{ path, branch, head, isBare, isDetached, isPrunable }]` |

Files in `status` look like
`{ path, oldPath, status, isStaged, isUnstaged, isBinary, additions, deletions }`.
Diff rows look like `{ kind, oldLineNumber, newLineNumber, oldText, newText, text }`,
where `kind` is `hunk`, `context`, `addition`, or `deletion`.

## Changing

| Call | Options |
| --- | --- |
| `stage`, `unstage` | `paths` (empty means all) |
| `discard` | `paths`, `untrackedPaths` |
| `commit` | `message`, `stageAll`. Returns `{ hash }` |
| `push` | `setUpstream` |
| `pull` | |
| `checkout`, `cherryPick`, `revert` | `hash` |
| `init` | |
| `branch.create` | `name` |
| `branch.switchTo` | `branch` |
| `branch.delete` | `name`, `force` |
| `branch.deleteRemote` | `branch` |
| `tag.create` | `name`, `hash` |

## Worktrees

| Call | Options |
| --- | --- |
| `worktree.add` | `path`, `branch`, `createBranch`, `baseBranch`. Returns the new folder |
| `worktree.remove` | `path`, `force`. Refuses uncommitted changes unless `force` |
| `worktree.switchTo` | `identifier` |

Paths are relative to the project folder, or start with `~/` for the home
folder.

## Pull requests

Pull request calls use the `gh` CLI on the project's server.

| Call | Options |
| --- | --- |
| `pr.info` | Returns the branch's pull request, or `null` |
| `pr.number` | Returns its number, or `null` |
| `pr.list` | `filter` (`open`, `closed`, `merged`, `all`), `limit` (100), `checks` |
| `pr.diff` | `number`, `lineLimit` |
| `pr.create` | `title`, `body`, `baseBranch`, `draft` |
| `pr.merge` | `number`, `method` (`merge`, `squash`, `rebase`), `deleteBranch` (default `true`) |
| `pr.close`, `pr.checkout` | `number` |
| `pr.checkoutWorktree` | `number`, `path` |

A pull request looks like `{ number, url, title, author, headBranch, headOid,
baseBranch, state, isDraft, updatedAt, mergeable, mergeStateStatus,
isCrossRepository, checks }`, where `checks` is
`{ passing, failing, pending, total, status }`.

`muxy.gh.user()` (needs `gh:read`) returns the signed-in GitHub user.

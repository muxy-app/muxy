# Files

`muxy.files` gives extensions read/write access to the **active project workspace** — list a directory, read a file, stat, and create, write, rename, move, or delete entries. This is the API surface that replaces the old built-in file tree: build your own tree, picker, or editor on top of it.

All methods are async (return a `Promise`). Every `path` is **relative to the active worktree root** (the same root the app shows for the active project). Pass `{ project }` (a project id, name, or path) as the last argument to target a specific project; omit it to use the active one.

Paths are sandboxed to the workspace root. Any path that escapes it — via `..` or a symlink pointing outside — is rejected.

## Permissions

| Permission | Methods |
| --- | --- |
| `files:read` | `list`, `read`, `stat` |
| `files:write` | `write`, `mkdir`, `rename`, `move`, `delete` |

Every **write** also prompts the user for [runtime consent](permissions.md#runtime-consent) the first time, remembered as an allow/deny rule per operation for the extension.

```json
{
  "name": "files-tree",
  "version": "0.1.0",
  "permissions": ["files:read", "files:write"]
}
```

## Read methods

### `muxy.files.list(path, opts?)`

```js
const entries = await muxy.files.list("src");
// [{ name, path, isDirectory, isIgnored }]
```

Directories sort before files. `path` is relative to the root; pass `""` or `"."` for the root itself. `isIgnored` reflects `.gitignore`.

### `muxy.files.read(path, opts?)`

```js
const file = await muxy.files.read("README.md");
// { path, content, size }
```

Reads UTF-8 text. Files larger than 5 MB or non-UTF-8 content reject.

### `muxy.files.stat(path, opts?)`

```js
await muxy.files.stat("src/main.swift");
// { name, path, isDirectory, size }
```

## Write methods

All writes prompt for consent on first use.

```js
await muxy.files.write("notes/todo.md", "# Todo\n");   // overwrite/create => { path }
await muxy.files.mkdir("notes");                        // => { path }
await muxy.files.rename("todo.md", "done.md");          // => { path }
await muxy.files.move(["a.txt", "b.txt"], "archive");   // => [path, path]
await muxy.files.delete(["old.log"]);                   // moves to Trash
```

- `write` does not create parent directories — call `mkdir` first.
- `rename` and `move` keep any open editor tabs pointed at the moved files.
- `delete` moves entries to the system Trash, not a permanent removal.

## Watching for changes

Subscribe to the [`file.changed` event](events.md) (from a background script) to react when the workspace changes on disk:

```js
muxy.events.subscribe("file.changed", ({ path, projectPath }) => {
  // refresh your tree
});
```

## Errors

A rejected promise carries a message string:

- `permission denied (files:read|files:write)` — missing manifest permission.
- `user denied consent for files.<op>` — the write consent prompt was denied.
- `path '…' escapes the workspace root` — the path resolved outside the sandbox.
- `project not found …` — the `project` selector did not resolve.
- Anything else surfaces the underlying filesystem error text.

## Notes

- `muxy.files` is available to extension **tabs**, **panels**, and **popovers**. Background scripts get `file.changed` events but not the `files.*` calls.
- The sandbox is the active worktree root; switching the active project/worktree changes what `muxy.files` sees.

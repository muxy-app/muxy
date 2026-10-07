# Files

Read and change files in a project. Calls run on the project's server, so they
work the same for local and remote projects. Available on pages and in scripts.

Paths are relative to the project folder. A tab works on its own project;
other views work on the current project. Pass `{ project }` to target another
project or worktree by ID.

| Call | Needs | Returns |
| --- | --- | --- |
| `files.list(path)` | `files:read` | `[{ name, path, isDirectory, isIgnored }]` |
| `files.read(path)` | `files:read` | `{ path, content, size }` |
| `files.stat(path)` | `files:read` | `{ path, name, isDirectory, size }` |
| `files.write(path, contents)` | `files:write` | |
| `files.mkdir(path)` | `files:write` | |
| `files.rename(path, newName)` | `files:write` | `{ path }` |
| `files.move(paths, into)` | `files:write` | new paths |
| `files.delete(paths)` | `files:write` | |

```js
const entries = await muxy.files.list("");
const readme = await muxy.files.read("README.md");
await muxy.files.write("notes/todo.md", readme.content + "\n- [ ] ship it");
```

- Writes ask the user first; see [Permissions](permissions.md#runtime-prompts).
- `delete` moves files to the Trash.
- Reads and writes are limited to 5 MiB, and listings to 16,384 entries.
- Subscribe to `file.changed` (with `files:read`) to hear about changes; see
  [Events](events.md).

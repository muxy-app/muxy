# Projects

A project is a folder on a server. You work in its tabs, and its terminals
belong to it. Projects are shared by every app on that server; tabs, splits,
workspaces, and project order are kept by each app.

## Projects

- Add one with **Add Project** at the bottom of the sidebar, **File → Open
  Project…**, or `Cmd+O`. Type to search folders, or type a path. With remote
  servers set up, choose **Local** or a server.
- `muxy <folder>` in a shell opens a folder in the desktop app.
- Right-click a project to rename it, or change its icon, logo, or color.
- **Remove Project…** removes it from every app and ends its terminals. Files
  stay on disk.
- If a project's folder disappears, it shows as missing and can only be
  removed.

**Home** is the built-in project for your home folder. It is always first and
can't be removed. `Cmd+N` opens a tab in Home.

## Sidebar

- `Cmd+B` shows or hides the sidebar.
- Pick a layout from the sidebar header:
  - **Project Focused** (default): projects, with worktrees under them.
  - **Tab Focused**: projects with their tabs listed in the sidebar.
  - **Agents Focused**: only tabs running an [AI agent](ai-agents.md).
- Sort by **Manual Order** and drag to reorder, or by **Name**.
- `Ctrl+1`–`Ctrl+9` jumps to a project. Hold a modifier to see the numbers.

## Workspaces

A workspace is a named filter for the sidebar. Create one from the workspace
menu in the sidebar header. Add projects from a project's **Workspaces** menu.
A project can be in any number of workspaces. Deleting a workspace never
removes its projects.

## Worktrees

For Git projects, worktrees appear under their project and have their own
tabs. Turn them on with **Worktrees → Show Worktrees** in the project menu.
Worktrees created outside Muxy show up on their own.

**New Worktree…** asks for:

- A name, and a new branch (from a base branch) or an existing one.
- A location: **Default** (`~/.muxy/worktrees/<project>/<name>`), a
  **Template**, or a **Folder**. Templates must contain `{branch}` and can use
  `{base-dir}` and `{project-name}`, for example `../{base-dir}.{branch}`.
  Set the default in **Settings → General**; each project remembers its choice.

**Remove Worktree and Files…** deletes the worktree folder and its project, and
ends its terminals. Muxy warns about uncommitted changes.

### Setup and teardown commands

Muxy can run commands when it creates or removes a worktree. List them in
`.muxy/worktree.json` in the project, or for every project in
`~/.config/muxy/worktree.json` (`$XDG_CONFIG_HOME/muxy/worktree.json`):

```json
{
  "setup": ["npm ci", { "name": "Start services", "command": "docker compose up -d" }],
  "teardown": ["docker compose down"]
}
```

- Muxy shows the commands and runs them only if you turn them on in the dialog.
- Setup runs your own commands first, then the project's. Teardown runs the
  project's first.
- Commands run in the worktree with `MUXY_PROJECT_PATH`, `MUXY_WORKTREE_ID`,
  `MUXY_WORKTREE_PATH`, `MUXY_WORKTREE_NAME`, and `MUXY_WORKTREE_BRANCH`. They
  share a five-minute limit.
- A failed setup keeps the new worktree. A failed teardown stops the removal.

## Layouts

Check a `.muxy/layouts/` folder into a project to open a ready-made set of
split terminals. See [Layouts](../layouts/overview.md).

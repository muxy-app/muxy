# Project Workspaces

Project workspaces are named filters for the sidebar. They help keep large project lists short.

They do not move files or change project data.

## Use them

Open the workspace menu at the top of the sidebar.

| Action | How |
| --- | --- |
| Show everything | Pick **All Projects** |
| Switch from the keyboard | Press **⌘⌥S** to open the workspace switcher in the terminal omnibox |
| Create a workspace | Pick **New Workspace** |
| Rename / delete | Use the workspace row actions |
| Move a project | Right-click a project -> **Move to Workspace** |

A project can belong to one workspace at a time.

Remote workspaces use reusable SSH devices from Settings. New SSH devices export `TERM=xterm-256color` by default, and the device's advanced environment settings are applied before remote terminals, git, files, worktrees, and extension commands run.

## Persistence

Workspace groups are saved in:

```text
~/Library/Application Support/Muxy/project-groups.json
```

The active workspace selection is saved in user defaults.

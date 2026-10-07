# Extensions Overview

> **New here?** Start with [Get started](get-started.md).

An extension is a web project that adds to the desktop app: tabs, panels,
popovers, a sidebar, top bar and status bar items, palette commands, shortcuts,
background logic, and app languages. You build it with any web tooling and Muxy
loads the build output.

Most extensions made for Muxy 1.x work unchanged. A few 1.x features are not
available yet; see [Coming from 1.x](get-started.md#coming-from-1x).

## Where code runs

| Where | What it is | API |
| --- | --- | --- |
| **Pages** | Tabs, panels, popovers, the sidebar, and modal web views. Native WebKit views that stay alive while hidden. | `window.muxy`, async (Promises) |
| **Scripts** | A `runScript` command. Runs once in JavaScriptCore on its own thread. | `muxy`, synchronous |
| **Background** | The optional `background` script. Runs while the extension is enabled. Receives events. | `muxy`, synchronous |

Not every API exists everywhere. See the [API reference](api.md#availability).

File, Git, and `exec` work runs on the project's server, so it works the same
for local and remote projects. Web requests run in the desktop app.

## Pages

| Page | What's in it |
| --- | --- |
| [Get started](get-started.md) | Build, load, and debug an extension |
| [Manifest](manifest.md) | `package.json` fields |
| [Permissions](permissions.md) | Permissions and runtime prompts |
| [API reference](api.md) | Every `muxy` namespace and where it is available |
| [Events](events.md) | Workspace events and extension messages |
| [Tabs](tabs.md) | Tab types, file openers, page data, and theme |
| [Panels](panels.md) | Panels, popovers, and the sidebar |
| [Bar items](bar-items.md) | Top bar and status bar items |
| [Commands](commands.md) | Palette commands, shortcuts, and scripts |
| [Dialogs](dialogs.md) | Dialogs, pickers, and modal web views |
| [Files](files.md) | Reading and writing project files |
| [Git](git.md) | Repository, branches, pull requests, and worktrees |
| [Localizations](localizations.md) | Language packs |
| [Contributing](contributing.md) | Publish to the marketplace |

## Where extensions live

Installed extensions live in the `extensions/<name>/` folder of Muxy's
[profile folder](../features/server.md#files). Each is the build output, with
`package.json` at its root.

Manage them in **Settings → Extensions**: **Installed** lists them with their
permissions, settings, and logs; **Browse** installs from the marketplace.
Extensions start disabled after installing or loading.

## Security

- **Declared permissions.** Every API call that reads or changes something
  needs a [permission](permissions.md) in the manifest.
- **Runtime prompts.** Running programs, typing into terminals, writing files or
  Git, web requests, and deleting projects ask the user, even with the
  permission.
- **Declared events.** An extension only receives events it lists in its
  manifest.
- **Own files only.** Pages load only from the extension's own folder.
- **Verified installs.** Marketplace packages are checked against their listing
  before they are installed.

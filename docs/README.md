# Muxy Documentation

> **Reading this as an LLM?** Start from <https://muxy.app/llms.txt> — an index of every page that links to its raw Markdown source. Append `/plain` to any docs URL (e.g. <https://muxy.app/docs/extensions/manifest/plain>) for that page's raw Markdown.

Muxy's maintained documentation covers terminal automation, declarative layouts, extensions, and the remote server API.

## CLI

| Page | What's in it |
| --- | --- |
| [Muxy CLI](cli/README.md) | Open projects and control workspaces, tabs, panes, and browser sessions |

## Layouts

| Page | What's in it |
| --- | --- |
| [Layouts Overview](layouts/overview.md) | Declarative `.muxy/layouts/*.yaml` workspaces |
| [Layout Schema](layouts/schema.md) | Fields, single panes, split trees, JSON form |
| [Layout Examples](layouts/examples.md) | Ready-to-adapt layout recipes |

## Extensions

> **DEV — under active development.** APIs and manifest format may change.

**Start here**

| Page | What's in it |
| --- | --- |
| [Get started](extensions/get-started.md) | Build and run your first extension in ~2 minutes |
| [Extensions Overview](extensions/overview.md) | Runtime model and security boundaries |
| [Manifest](extensions/manifest.md) | `package.json` fields and background environment |
| [Permissions](extensions/permissions.md) | Permission grants and runtime consent |

**Build UI**

| Page | What's in it |
| --- | --- |
| [Extension Tabs](extensions/tabs.md) | Render a full tab as a webview, `window.muxy` bridge |
| [Browser](extensions/browser.md) | Drive built-in browser tabs from extensions |
| [Extension Panels](extensions/panels.md) | Docked or floating webview beside the workspace |
| [Extension Popovers](extensions/popovers.md) | Transient webview anchored to a topbar or status bar item |
| [Extension Sidebars](extensions/sidebars.md) | Replace the built-in sidebar with an extension webview |
| [Topbar Items](extensions/topbar.md) | Add an icon to the tab-strip button cluster |
| [Status Bar Items](extensions/statusbar.md) | Add an icon and optional text to the footer status bar |
| [Extension Modal](extensions/modal.md) | Native searchable picker and webview modal overlays |
| [Extension Dialogs](extensions/dialogs.md) | Native confirm, alert, prompt, and folder sheets |
| [Palette Commands](extensions/palette-commands.md) | Register palette commands and shortcuts |
| [Inline Scripts](extensions/scripts.md) | Run palette commands in an in-process JS context |
| [Lifecycle](extensions/lifecycle.md) | Intercept tab, panel, and popover closes |

**Work with the workspace**

| Page | What's in it |
| --- | --- |
| [Events](extensions/events.md) | Workspace and extension-local events |
| [Git](extensions/git.md) | Repository operations against the active worktree |
| [GitHub](extensions/gh.md) | Read the authenticated GitHub CLI user |
| [Files](extensions/files.md) | Sandboxed workspace filesystem operations |
| [HTTP](extensions/http.md) | CORS-free external requests with host consent |
| [Storage](extensions/storage.md) | Persistent extension-scoped JSON storage |
| [Settings](extensions/settings.md) | Typed settings with a Settings sidebar row |
| [Remote Methods](extensions/remote-methods.md) | Serve named methods to the Muxy mobile app |
| [Extension Logs](extensions/logs.md) | Per-extension logs and rotation behavior |

**Publish**

| Page | What's in it |
| --- | --- |
| [Contributing an extension](extensions/contributing.md) | Build, validate, and publish an extension |

## Agent Skills

Muxy ships two [skills.sh](https://www.skills.sh) skills that teach coding agents its conventions:

| Skill | What it teaches | Install |
| --- | --- | --- |
| `muxy-cli` | Driving the workspace from a shell — see [Muxy CLI](cli/README.md) | `npx skills add github.com/muxy-app/muxy/tree/main/Muxy/Resources/skills/muxy-cli` |
| `muxy-extension` | Authoring extensions — see [Get started](extensions/get-started.md) | `npx skills add github.com/muxy-app/muxy/tree/main/Muxy/Resources/skills/muxy-extension` |

## Remote Server

| Page | What's in it |
| --- | --- |
| [Remote Server Overview](remote-server/overview.md) | WebSocket API for mobile clients |
| [Setup & Security](remote-server/setup.md) | Enable the server, port, security model |
| [Pairing & Authentication](remote-server/pairing.md) | Authenticate, pair, register flow |
| [Protocol](remote-server/protocol.md) | Message envelope, request/response/event |
| [API Methods](remote-server/methods.md) | Every RPC method and its parameters |
| [Events](remote-server/events.md) | Server-pushed events and their payloads |
| [Data Objects](remote-server/data-objects.md) | Project, worktree, workspace, notification, terminal snapshot |

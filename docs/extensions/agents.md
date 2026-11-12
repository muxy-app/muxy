# Build with an agent

Muxy can hand extension work to a coding agent installed on your computer. The
agent writes the extension, loads it into the running app, and fixes what the
logs report, so it is usable as soon as the agent finishes.

## Create an extension

1. Click **Extensions** at the bottom of the sidebar and choose **Create
   Extension…**, or run it from the command palette.
2. Pick an agent. The list shows the
   [supported agents](../features/ai-agents.md) installed on this computer, with
   the one you used last at the top.
3. If the `muxy` command isn't installed, Muxy asks to install it. Agents need it
   to load what they build, so declining stops here.
4. A terminal tab opens with the agent running. Tell it what you want.

The agent loads and enables the extension itself. Some agents can't be started
with a first message. For those, the terminal prints the text to paste.

## The workspace

Agents work in one folder, kept apart from your installed extensions:

| Build | Folder |
| --- | --- |
| Stable | `~/.config/muxy/extension-workspace` |
| Beta | `~/.config/muxy-beta/extension-workspace` |
| Local build | `~/.config/muxy-dev/extension-workspace` |

`$XDG_CONFIG_HOME` replaces `~/.config` when it is set, and `$MUXY_DIR` holds the
folder directly when it is set.

```
extension-workspace/
  extensions/<name>/     one folder per extension
  shared/                the store's shared UI, once a clone needs it
  .claude/skills/        the muxy-extensions skill
  .agents/skills/        the same skill for other agents
  AGENTS.md, CLAUDE.md   point other agents at the skill
```

Muxy replaces its own skill and the `AGENTS.md` and `CLAUDE.md` pointers whenever
an update changes them. Your extensions and any other skills you add stay
untouched. Extensions load from here as **Unpacked**, so **Unload folder** keeps
your files.

## Change an extension

In **Settings → Extensions**, an unpacked extension has **Edit with Agent…**. A
folder outside the workspace stays where it is, and the agent works there without
adding Muxy's files to it.

An extension from the marketplace has **Clone and Modify…** instead. Muxy copies
its source to `extensions/<name>-custom/` under that new name, and your agent
loads the copy when it works. The original stays installed until you remove it.
Choosing **Clone and Modify…** again returns to the same copy. If the source
can't be downloaded, Muxy copies the installed build output instead.

## The `muxy extension` commands

Agents use these, and so can you. They need the desktop app on this computer, and
start it if it isn't running. A relative folder is read from where you run the
command, so these examples run from the workspace. See
[Muxy CLI](../features/muxy-cli.md).

```bash
muxy extension load extensions/my-extension
muxy extension enable my-extension
muxy extension reload my-extension
muxy extension status my-extension
muxy extension logs my-extension --lines 50
```

`reload` restarts only that extension. If its folder no longer loads, or its
package name changed, you get the reason and the running copy stays as it was.

Agents also check their work in the running app, the way you would use it:

```bash
muxy extension dev extensions/my-extension          # build and reload on every change
page=$(muxy extension open my-extension panel:main)  # open a panel, tab, or modal
muxy extension page click "$page" "#save"           # drive the page like a browser tab
muxy extension page screenshot "$page" --out shot.png
muxy extension emit my-extension file.changed '{"path":"README.md"}'
```

See [Developing and testing](../features/muxy-cli.md#developing-and-testing)
for every command. TypeScript types for the `muxy` API ship with the skill, in
`references/types/muxy.d.ts`.

There is deliberately no command to uninstall a marketplace extension, or to
change what an extension may do. [Runtime prompts](permissions.md#runtime-prompts)
still ask you, whoever loaded the extension. While one waits, the extension's
log says so, so the agent can tell you what it is waiting for.

## Publish

See [Contributing](contributing.md#publish-from-muxy).

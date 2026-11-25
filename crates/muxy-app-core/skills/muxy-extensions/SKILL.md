---
name: muxy-extensions
description: Build, change, debug, and publish Muxy extensions, and load them into the running Muxy app with the `muxy extension` commands. Use this for any work on a Muxy extension, including a package.json with a `muxy` block.
---

# Muxy extensions

A Muxy extension is a folder with a `package.json` whose `muxy` block declares
what it adds: tabs, panels, popovers, commands, bar items, settings, and a
background script. Its pages are plain web pages with a `window.muxy` API.
Panels can also float freely over the window, several at once, framed or
frameless, transparent, and click-through: sticky notes, widgets, or animated
pets. `references/panels.md` covers them.

## Where the work goes

- New extensions go in `extensions/<name>/` of this workspace. The folder name
  must equal the package `name`.
- Names use only `a-z`, `0-9`, `.`, and `-`. Pages don't load otherwise.
- If you were pointed at a folder outside this workspace, work there instead,
  and don't add this skill or other Muxy files to it.

## The loop

1. Ask what the extension should do, unless the user already said. Keep it to
   the questions that change what you build.
2. Write the extension. Read the references below instead of guessing at
   manifest fields or API names, and ask for the fewest permissions that work.
   For TypeScript, copy `references/types/muxy.d.ts` into the extension; it
   types `window.muxy`, and the type check catches wrong API names.
3. Build, load, enable, and reload it in one step:

   ```bash
   muxy extension dev extensions/my-extension --once
   ```

   It runs the `build` script if there is one, so a stale `dist/` can't hide
   your edits, then prints `ready` or `failed` with the load errors and the log
   lines that round added. Without `--once` it keeps watching and does another
   round after every change; run it in a terminal of its own, such as a
   `muxy new` session, and read that output instead of guessing.
4. Check that it works by using it, not just by loading it:

   ```bash
   page=$(muxy extension open my-extension panel:main)
   muxy extension page snapshot "$page"
   muxy extension page click "$page" "#save"
   muxy extension page wait "$page" --text "Saved"
   muxy extension page screenshot "$page" --out /tmp/my-extension.png
   ```

   `open` takes `panel:<id>`, `tab:<type>` or `modal:<entry>` and prints the
   page ID once the page has loaded. `pages` lists the pages already open.
   `page` takes the same actions as `muxy browser`: `snapshot` lists the
   interactive elements with selectors, `eval` runs JavaScript in the page and
   prints its result, and `screenshot --out` saves a PNG you can look at. Look
   at it in both light and dark themes when the extension draws its own
   colors.
5. Reach the rest the way the user would. `run` runs one of its commands,
   and `emit` sends it an event its manifest lists, such as `file.changed`,
   with a JSON payload, so you can see how it reacts:

   ```bash
   muxy extension run my-extension toggle
   muxy extension emit my-extension file.changed '{"path":"README.md"}'
   ```
6. Tell the user how to open it.

Don't declare it finished while `dev` prints `failed`, the log shows an
uncaught error, or what you checked in step 4 didn't happen.

### Runtime prompts

Running programs, writing files or Git, web requests, and a few other calls
ask the user first, each time, until they choose to remember it. While a call
waits, the log says `... is waiting for the user to allow it in Muxy`, and
`muxy extension status my-extension` counts the prompts still waiting. Tell the user what
to answer and why; you can't answer for them. A prompt nobody answers is
denied after 60 seconds, and the log says so.

## Commands

These talk to the Muxy app on this computer, and start it if needed. Add
`--json` to `list`, `status`, `pages`, `validate`, or `publish` for structured
output.

```bash
muxy extension list
muxy extension status my-extension
muxy extension logs my-extension --lines 100
muxy extension dev extensions/my-extension
muxy extension dev extensions/my-extension --once
muxy extension load extensions/my-extension
muxy extension enable my-extension
muxy extension disable my-extension
muxy extension reload my-extension
muxy extension unload my-extension
muxy extension pages my-extension
muxy extension open my-extension panel:main
muxy extension open my-extension tab:viewer --data '{"file":"README.md"}'
muxy extension run my-extension toggle
muxy extension emit my-extension settings.changed '{"key":"greeting"}'
muxy extension page read 12
muxy extension page eval 12 document.title
muxy extension page screenshot 12 --out shot.png
muxy extension validate my-extension
muxy extension publish my-extension
```

- `dev` loads an unpacked folder that isn't loaded yet and turns it on.
- `load` remembers an unpacked folder. It starts disabled, so `enable` it.
- `reload` restarts only that extension. If the folder no longer loads, or its
  package name changed, it prints why and the running copy stays as it was.
- `unload` forgets an unpacked folder and keeps its files. It doesn't work on
  extensions installed from the marketplace.
- `page` takes every page action `muxy browser` has except opening,
  navigating, and cookies; run `muxy help extension page`.
- There is no command to uninstall a marketplace extension, or to change what
  an extension may do. If the user wants that, they do it in **Settings →
  Extensions**.

If `muxy` isn't found, tell the user to run **Install Command Line Tool…** from
Muxy's command palette, and stop until it works.

## Changing a copy of a marketplace extension

A clone is the user's own copy, so it must keep its new package name. The
original can stay installed, but if both define the same shortcut or commands,
suggest disabling the original with `muxy extension disable <original>`. Keep
the original's license and credit its author in the README.

Marketplace extensions may import `@muxy/ui` through a Vite alias that points at
`../../shared/ui`. Muxy copies `shared/` beside `extensions/` when it clones one,
so those builds keep working.

## Publishing

Publishing opens a public pull request against `muxy-app/extensions` from the
user's GitHub account. Maintainers review it, and once merged it is listed for
everyone. Published versions never change.

**Before running any publish command, explain this to the user and wait for an
explicit yes:** Muxy will fork `muxy-app/extensions` to their account if needed,
create a branch named `<name>-<version>`, commit the extension's source, push it
to their fork, and open a pull request titled `<name> <version>`.

1. Run `muxy extension validate my-extension` and fix what it reports. The store
   needs a semantic version, a `README.md`, a `package-lock.json` (run
   `npm install`), a `scripts.build` script, and a `marketplace` block with an
   icon and at least one screenshot. Details are in
   `references/contributing.md`. Make screenshots of pages with `page
   screenshot --out`, as in [the loop](#the-loop); ask the user for any you
   can't make, such as one of the whole window.
2. Bump `version` if this updates something already published.
3. Run `muxy extension publish my-extension`. Muxy asks the user to approve in
   its own window, and prints the pull request's address when it is open. It sends
   the files as they were when it asked, so later edits need another publish.
   Publishing the same version again updates that pull request, replacing its
   earlier commits but not its text.
4. Offer to improve the pull request's text with `gh pr edit`, especially why
   each permission is needed.

### Without the GitHub CLI

If `validate` or `publish` says `gh` is missing or signed out, guide the user
instead:

1. Explain what publishing does, as above, before anything else.
2. Help them install `gh` (`brew install gh` on macOS, or
   <https://cli.github.com>).
3. Ask them to run `gh auth login` themselves. It is interactive, and they must
   be the one to sign in.
4. Run `muxy extension publish my-extension` again.

If they can't use `gh` at all, walk them through the manual steps in
`references/contributing.md`, confirming each step that changes their GitHub
account before you run it.

## References

Read the one you need from `references/`, beside this file:

| File | Covers |
| --- | --- |
| `overview.md` | How extensions work, and where they run |
| `get-started.md` | A complete small extension |
| `manifest.md`, `schema/manifest.schema.json` | Every manifest field |
| `types/muxy.d.ts` | TypeScript types for the `muxy` API |
| `permissions.md` | Permissions and runtime prompts |
| `api.md` | The `window.muxy` API |
| `tabs.md`, `panels.md`, `bar-items.md`, `commands.md`, `dialogs.md` | Each kind of surface |
| `files.md`, `git.md`, `events.md`, `localizations.md` | Files, Git, events, and translations |
| `agents.md` | Building extensions with an agent, from the user's side |
| `contributing.md` | Publishing rules, by hand or with Muxy |

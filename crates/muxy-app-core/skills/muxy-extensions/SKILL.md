---
name: muxy-extensions
description: Build, change, debug, and publish Muxy extensions, and load them into the running Muxy app with the `muxy extension` commands. Use this for any work on a Muxy extension, including a package.json with a `muxy` block.
---

# Muxy extensions

A Muxy extension is a folder with a `package.json` whose `muxy` block declares
what it adds: tabs, panels, popovers, commands, bar items, settings, and a
background script. Its pages are plain web pages with a `window.muxy` API.

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
3. If it has a build step, run it. Muxy loads `dist/` when it exists, so a stale
   `dist/` hides your edits.
4. Load and enable it, the first time:

   ```bash
   muxy extension load extensions/my-extension
   muxy extension enable my-extension
   ```

5. After later edits, build if needed and reload:

   ```bash
   muxy extension reload my-extension
   ```

6. Check that it works, then tell the user how to open it:

   ```bash
   muxy extension status my-extension
   muxy extension logs my-extension --lines 50
   ```

Don't declare it finished while `status` shows a load error or the logs show an
uncaught error.

## Commands

These talk to the Muxy app on this computer, and start it if needed. Add
`--json` to `list`, `status`, `validate`, or `publish` for structured output.

```bash
muxy extension list
muxy extension status my-extension
muxy extension logs my-extension --lines 100
muxy extension load extensions/my-extension
muxy extension enable my-extension
muxy extension disable my-extension
muxy extension reload my-extension
muxy extension unload my-extension
muxy extension validate my-extension
muxy extension publish my-extension
```

- `load` remembers an unpacked folder. It starts disabled, so `enable` it.
- `reload` restarts only that extension. If the folder no longer loads, or its
  package name changed, it prints why and the running copy stays as it was.
- `unload` forgets an unpacked folder and keeps its files. It doesn't work on
  extensions installed from the marketplace.
- There is no command to uninstall a marketplace extension. If the user wants
  that, they do it in **Settings → Extensions**.

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
   `references/contributing.md`. Ask the user for screenshots you can't make.
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
| `permissions.md` | Permissions and runtime prompts |
| `api.md` | The `window.muxy` API |
| `tabs.md`, `panels.md`, `bar-items.md`, `commands.md`, `dialogs.md` | Each kind of surface |
| `files.md`, `git.md`, `events.md`, `localizations.md` | Files, Git, events, and translations |
| `agents.md` | Building extensions with an agent, from the user's side |
| `contributing.md` | Publishing rules, by hand or with Muxy |

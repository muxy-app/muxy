---
name: e2e-test
description: Run Muxy's end-to-end release tests in fresh SwiftVM guests (macOS, Ubuntu Wayland, Ubuntu X11), driven visually by parallel low-cost tester subagents. Use before a release, after a risky change, or when the user runs /e2e-test [macos|wayland|x11|linux].
argument-hint: "[macos|wayland|x11|linux|windows]  (empty = macOS and Linux)"
---

# Muxy E2E tests

You are the **coordinator**. You never drive a VM yourself. Low-cost tester
subagents boot the guests, build the current working tree, and run the
suites in `suites/` through Agent Desktop (keys, typing, clicks, screenshots)
and Muxy's CLI. You plan the run, dispatch testers, review their evidence,
and report.

Other skills reuse this harness for one scenario: write the steps as a suite
file in the same format, and hand it to testers with the prompt in step 4.
The tester protocol carries every rule for driving the VM, including Agent
Desktop, so don't write VM rules into a tester's prompt.

Read first: `docs/product/README.md`, `docs/tech/README.md`, and
[the tester protocol](references/tester.md) (you hand it to every tester).

## 1. Scope

| Argument | Platforms |
| --- | --- |
| empty, `all` | `macos`, `wayland`, `x11` |
| `linux` | `wayland`, `x11` |
| `macos`, `wayland`, `x11`, `windows` | that one |

Anything else: stop and show this table.

## 2. Prepare (host only, two commands)

```sh
.agents/skills/e2e-test/scripts/e2e preflight
.agents/skills/e2e-test/scripts/e2e prepare        # prints the run id, e.g. r10302015
```

`preflight` fails when a template is missing: follow
[references/templates.md](references/templates.md) before testing. It also
fails when the host DHCP pool for SwiftVM is full. Each new VM takes a lease,
and macOS never gives an expired lease to a new MAC, so the pool runs out after
about 250 VMs. A guest then gets no IPv4 address, and its build can't reach
github.com. Ask the user to run `e2e dhcp-prune` in their own terminal, because
it needs sudo. `up` also stops with `ENV:` when a guest has no network. `up`
also fails with `ENV:` when Agent Desktop is missing, not ready, or can't
take screenshots in the clone; that is a template defect, fixed by
refreshing the template. If `preflight` lists VMs from an earlier run, ask
the user whether to clean them up (`e2e clean <old run>`) before starting;
they hold host memory and disk.
`prepare` exports the working tree (tracked edits and new files included) to
`target/e2e/<run>/source.tar`; `prepare <run> <checkout>` exports another
checkout, such as a worktree. If you edit source later, run a new
`prepare` and a new run; never mix sources within a run.

## 3. Pick the tester model

Choose the cheapest model that has a shell and can **read PNG images**; the
tests are mechanical and need no reasoning beyond comparing what is seen with
what is written. Prefer, in order:

| Harness | Tester |
| --- | --- |
| Claude Code | `model: "haiku"` on a general-purpose subagent |
| Codex / ChatGPT agents | `gpt-6-luna` (or the cheapest listed `mini` model), low reasoning effort, `fork_turns: "none"` |
| Anything else | the cheapest available model that has shell and image input |

Set the model per subagent; never change the user's global settings. If no
cheaper model exists, use your own model for testers, but still delegate.

## 4. Dispatch

Each platform has its own VM and runs its batches **in order**; the three
platforms run **in parallel**. One tester per batch, fresh context each time.
Never let two testers touch the same VM at once.

| Batch | Suites | Notes |
| --- | --- | --- |
| A | boot, S01, S02 | Boots the VM with `e2e up` |
| B | S03, S04 | |
| C | S05, S06 | |
| D | S07, S08 | |
| E | S09, S10 | |
| F | S11 | |
| G | S12, S14 | Terminal UI and terminal images |
| H | S13, S15 | Extensions, AI and agent dialogs, language packs |

Start batch A for every platform in one message. When a platform's batch
finishes, start that platform's next batch right away; don't wait for the
other platforms. If batch A reports `BOOT: FAIL`, stop that platform. However
a run ends, finish with step 6.

Give each tester exactly this prompt, filled in:

```text
You are a Muxy E2E tester. Work only through the commands in the tester protocol.
Read and follow: .agents/skills/e2e-test/references/tester.md
Run id: <run>    Platform: <macos|wayland|x11|windows>    VM: muxy-e2e-<run>-<platform>
Batch: <A-F>     Boot first: <yes for batch A, otherwise no>
Suites, in order: .agents/skills/e2e-test/suites/<file>.md, ...
Return your report as your final message.
Do not spawn agents, edit repository files, or debug Muxy's source.
```

## 5. Review

Testers' verdicts are evidence, not conclusions. Save each returned report
as `target/e2e/<run>/<platform>/report-<batch>.md`. For every report:

1. Read the summary block. Any `FAIL`, `BLOCKED` or `INSTRUCTION` row needs
   your review; a `PASS` row with no evidence path is treated as `BLOCKED`.
2. For each problem row, open its screenshots and log excerpts yourself and
   classify it:
   - **Product bug**: Muxy behaved against the suite's expectation and the
     docs. Keep it.
   - **Environment**: VM, template, or tooling failure. Fix the template or
     script if it is routine (see templates.md), otherwise report it.
   - **Test defect**: the suite's steps or expectation are wrong. Fix the
     suite after the run, and say so in the final report.
   - **Flaky**: re-run only that test once with a new tester (same VM,
     after its batch) and keep both results.
3. Spot-check one `PASS` screenshot per suite to keep testers honest.

## 6. Finish (mandatory)

A run is not finished until its VMs are gone. Do these steps in order, every
time, even when the run failed or was stopped early.

1. **Stop the VMs.** Once every batch has reported, stop each platform's VM.
   This keeps its disk for inspection and frees the host:

   ```sh
   .agents/skills/e2e-test/scripts/e2e down muxy-e2e-<run>-<platform>
   ```

2. **Report to the user:**
   - Source: commit, dirty file count, archive digest (`target/e2e/<run>/source.txt`).
   - A platform × suite table: pass / fail / blocked counts.
   - Each confirmed product bug: test id, platform(s), expected vs actual, the
     key screenshot and log paths, and how reproducible it was.
   - Environment problems and test defects, separately, with what you changed.
   - What is not covered (see the coverage table below).
   - The run's VMs, from `e2e vms <run>`.

3. **Ask the user before cleaning up.** Ask whether they want any of the
   run's VMs kept to inspect a failure or reproduce a fix, and wait for the
   answer. Don't delete anything before they reply. If they want some kept,
   say which VMs remain and ask again once they say they are done.

4. **Clean up.** When the user is done, delete every VM of the run and
   confirm none are left:

   ```sh
   .agents/skills/e2e-test/scripts/e2e clean <run>
   .agents/skills/e2e-test/scripts/e2e vms <run>
   ```

   `clean` copies the logs into `target/e2e/<run>/` first, so the evidence
   stays. Tell the user the VMs are deleted (they are in the Trash). Delete
   only this run's VMs; never another run's, a template, or a VM the run
   didn't create.

## Coverage

| Suite | Feature |
| --- | --- |
| [S01](suites/01-launch.md) | Build identity, first launch, server auto-start |
| [S02](suites/02-terminal.md) | Terminal I/O, Unicode and color, Ctrl+C, scrollback, find, paste, shell integration, copy, font size |
| [S03](suites/03-tabs-panes.md) | Splits, focus, zoom, close confirmation, tabs, rename, detach and Existing Terminals |
| [S04](suites/04-projects.md) | Add project, Git status, CLI sessions in the app, rename, switch, delete |
| [S05](suites/05-persistence.md) | Terminals outlive the app, restore on launch, server stop and reconnect, End All Sessions and Quit |
| [S06](suites/06-clients-agents.md) | CLI input into a visible terminal, terminal UI, shared sessions, program status, agent activity |
| [S07](suites/07-layouts-worktrees.md) | Project layouts, worktrees from CLI and UI |
| [S08](suites/08-settings.md) | Settings window, saved preferences, theme picker, sidebar, command palette |
| [S09](suites/09-browser.md) | Built-in browser, CLI automation, address bar, back |
| [S10](suites/10-composer.md) | Composer send |
| [S11](suites/11-worktrees.md) | Worktrees end to end: discovery, form and CLI creation, setup and teardown commands, location templates, register, switch, rename, dirty and clean removal, lost folders, hiding, removing the parent |
| [S12](suites/12-terminal-ui.md) | Terminal UI: isolated server, idle output limit, splits, suspend and resume, detach and restore, resize, two windows sharing one layout |
| [S13](suites/13-extensions.md) | Extensions: store listing, load, enable, panel, reload, broken package, disable and unload, floating panels |
| [S14](suites/14-terminal-images.md) | Terminal images: Kitty, iTerm and Sixel, file and temp-file transfers, the named-pipe guard, DA1, restore after restart, scrolling |
| [S15](suites/15-agents-language.md) | Commit dialog, Create Extension with an agent, Linux install of the `muxy` command, symbol language packs |

Not covered: remote servers and pairing (need a second machine), mobile,
`muxy worktree checkout-pr` (needs a GitHub remote),
Quick Terminal (needs a recorded global shortcut), dictation, extensions
marketplace (network), updates, AI commit/PR (needs signed-in tools),
Windows.

## Changing the suites

Write steps a model can follow without judgment: one action per step, exact
keys for both platforms, an exact expected result, and a check that doesn't
rely on typed text (see "Markers" in the tester protocol). Click with
`$E click-element $VM <name>`, using the control's accessible name (its
`aria_label`, or the English text of its `tr!` label), and use `$E click` only for what
isn't a control, saying why in that step. Never click by accessibility ID
(`project-row-alpha`, `settings-toggle-...`): `swiftvm control` reports only each
control's role, name and position, not Muxy's IDs, so an ID never matches. When
a name repeats, add `--role <role>` (as `look` prints it: `button`, `toggle`,
`textfield`, `item`, `tab`, ...; use the Linux names: `toggle` also
matches macOS's `checkbox`, and `tab` also matches macOS's `radio`) or `--nth <n>` (the nth match in tree order),
and say in the step why. `--right` finds the control by name and right-clicks it with the
pointer. `click-element` fails and lists the matches when a name is ambiguous. For a native dialog on macOS, ask
for `$E desktop $VM look --app muxy-app` as well as a shot (`swiftvm control`
has no `snapshot` action). Verify new steps
in a guest before committing them. Keep `references/tester.md` the single
place for shared rules.

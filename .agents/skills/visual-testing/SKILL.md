---
name: visual-testing
description: Test one feature or fix visually in fresh SwiftVM guests (macOS, Ubuntu Wayland, Ubuntu X11, Windows 11). Writes scenarios for just that change, has low-cost testers drive the real app, reviews the screenshots, then fixes and retests until it works. Use while building a feature or after a fix, to confirm a change works in the real app, or when the user runs /visual-testing. For a full release pass, use e2e-test instead.
argument-hint: "[macos|wayland|x11|linux|windows|all] [what to test]  (empty = the current change)"
---

# Muxy visual testing

Prove that one change works in the real app, on the platforms it can affect.
You are the **coordinator**: you decide what to test, write the scenarios, read
the evidence, and fix what is broken. Low-cost tester subagents drive the VMs.

This skill reuses the `e2e-test` harness and adds no VM tooling of its own:

- `E=.agents/skills/e2e-test/scripts/e2e` boots and drives the VMs.
- [The tester protocol](../e2e-test/references/tester.md) holds every rule for
  driving a VM. Read it before writing scenarios, and don't repeat its rules.
- [Templates](../e2e-test/references/templates.md) covers a missing or stale template.

Use `e2e-test` instead when the question is "does everything still work?".

## 1. Scope

**What to test.** Use the user's words if they gave any. Otherwise read the
request and the diff (`git status`, `git diff <base>`) and write a short list of
acceptance criteria. Ask the user only if you can't tell what "working" means.

**Where to test.** Take the first rule that applies, and state your choice:

| Rule | Platforms |
| --- | --- |
| The user named platforms | Those (`linux` = `wayland` and `x11`, `all` = all four) |
| The change is only compiled or only reachable on one OS | That OS (Linux = `wayland` and `x11`) |
| Anything else, including shared UI | `macos`, `wayland`, `x11`, `windows` |

Some features can't be tested in a guest, such as remote pairing or anything
that needs signed-in tools (see "Not covered" in the
[e2e-test skill](../e2e-test/SKILL.md)). Tell the user which criteria that leaves
unproven; never report them as passing.

## 2. Prepare

`.agents/` is ignored by Git, so it exists only in the main checkout. Run every
command, and start every tester, from the main checkout's root. From a worktree,
find it with `git worktree list` and pass the worktree to `prepare`.

```sh
cargo check -p muxy-app                       # A compile error shouldn't cost a VM boot
$E preflight
$E prepare                                    # This checkout; prints the run id
$E prepare "$(date +r%m%d%H%M)" <worktree>    # Another checkout
```

- `preflight` fails when a template is missing: follow the templates reference.
- Every task gets a new run and fresh VMs. Never reuse a VM from an earlier task,
  even for the same branch or feature. If `preflight` lists VMs from another run,
  leave them alone and ask the user whether to clean them up; another session may
  be using them.
- `prepare` copies tracked edits and new files, so nothing needs committing. It
  doesn't copy dependencies. GPUI comes from its Git fork, with `--locked`, so a
  fork change must be pushed and in `Cargo.lock` before the guest can build it.

## 3. Write the scenarios

Write them to `target/e2e/<run>/scenarios.md`, in the format of
[the e2e suites](../e2e-test/suites/08-settings.md), which also lets you copy any
existing test that fits. Read the source for real control IDs, names and shortcuts:
a tester can't find them and must not guess.

```markdown
# Visual test: <feature>

<One sentence: what this change should do.>

Start: run `$E g $VM reset`, then <exact steps to reach the feature>.
Tests continue from each other in order.

## V1 <Acceptance criterion>

1. <One action, with the exact command>.
2. **Shot** `v1-after`. Check: <exactly what must be visible, and where>.
```

Cover, in this order:

1. **The feature.** One test per acceptance criterion, each with a check that
   would fail without the change.
2. **What users will hit next.** Cancel and empty states, invalid input, a second
   use, and restarting the app where the state should persist.
3. **The neighbors.** One or two tests of existing behavior the diff touches.
   Stop there; broad coverage is `e2e-test`'s job.

Keep a file to about ten tests. If it needs more, split it into files that testers
run one after another on the same VM.

Things the harness doesn't do for you:

- **New shortcuts.** `$E press` knows only the shortcuts listed in `ACTIONS` in
  `scripts/e2e`. Add the new one there, for both platforms.
- **Setup.** `reset` wipes the profile (`~/.local/state/muxy-vm`) and recreates the
  fixtures (`~/e2e`: `alpha`, `beta`, `gamma`, `site`). Build any other state after
  it, in steps: run `$E g $VM muxy ...`, or type into a terminal, then
  `$E g $VM quit` and `$E g $VM launch` if the app reads it at startup.
- **Missing commands.** If a step needs something the protocol lacks, add it to
  `scripts/guest/muxy-e2e` and document it in the protocol's Commands table.
  `up` copies the helper to the guest each time.
- **Platform differences.** Say so in the test: `SKIP on wayland and x11.`
- **Windows.** The guest helper is `muxy-e2e.ps1`, with a subset of the
  commands (`$E g $VM help` lists them). Paths like `'~/e2e/beta'` still work.
  Its terminals run PowerShell 7, so write typed commands for it, and mark
  tests that need a missing fixture `SKIP on windows`. Muxy's controls aren't
  in SwiftVM's accessibility view there, so `click-element` can't find them:
  steps use shortcuts, keys and pixel clicks read from a screenshot.

## 4. Run

Pick the tester model as in step 3 of the [e2e-test skill](../e2e-test/SKILL.md):
the cheapest with a shell and image input (`model: "haiku"` in Claude Code). Without
subagents, follow the tester protocol yourself.

Start one tester per platform, in parallel, in one message. Never let two testers
touch the same VM at once. Give each exactly this prompt, filled in:

```text
You are a Muxy E2E tester. Work only through the commands in the tester protocol.
Working directory: <main checkout root>
Read and follow: .agents/skills/e2e-test/references/tester.md
Run id: <run>    Platform: <macos|wayland|x11|windows>    VM: muxy-e2e-<run>-<platform>
Batch: <attempt number, starting at 1>     Boot first: yes
Suites, in order: target/e2e/<run>/scenarios.md
Return your report as your final message.
Do not spawn agents, edit repository files, or debug Muxy's source.
```

`Boot first: yes` is right for every attempt. On attempt 1, `$E up` creates a fresh
VM. On later attempts, it imports the source from the latest `prepare` into that VM
and rebuilds incrementally.

## 5. Review

The screenshots decide, not the testers' verdicts. Save each report as
`target/e2e/<run>/<platform>/report-<attempt>.md`, then:

1. Open every screenshot that proves an acceptance criterion yourself, on each
   platform, including the `PASS` rows. Testers follow steps but miss wrong layout,
   clipped text, bad spacing and wrong colors, which is what this skill is for.
2. Check that the feature matches across platforms, apart from intended differences.
3. Classify each problem:
   - **Bug in this change**: fix it (step 6).
   - **Bug outside this change**: don't fix it. Report it to the user.
   - **Scenario defect**: the steps or expected result are wrong. Fix the scenario
     and rerun that test.
   - **Environment**: VM, template or tooling failed. Fix it per the templates
     reference if routine, otherwise report it.
   - **Flaky**: rerun that test once with a new tester and keep both results.

## 6. Fix and retest

Retests within a task use that task's VMs, so keep them running. Fix the bug, then
repeat on the same run:

```sh
$E prepare <run> [checkout]    # Same run id: replaces the source for this run's VMs
```

Then run step 4 again with the next attempt number. A pass counts only for the
source it ran on, so each attempt reruns the whole scenario file, on every platform
in scope. Rerun fewer platforms only when the fix is in code that compiles for just
one OS.

Change a test's expected result only when the expectation was wrong, never to get a
pass. Stop after three attempts at the same problem and report what you tried.

## 7. Finish (mandatory)

A task isn't done until its VMs are gone. Do both steps every time, including when
the task failed or was stopped early.

1. **Remove the VMs.** Don't ask first; keep one only if the user asked you to.
   `clean` stops and deletes every VM of the run, after copying the logs into
   `target/e2e/<run>/`. The screenshots stay there too.

   ```sh
   $E clean <run>
   $E vms <run>    # Must list no VMs
   ```

   Delete only this run's VMs; never another run's, or a template.

2. **Report to the user:**
   - The verdict: does the change work, and on which platforms?
   - A platform × acceptance-criterion table, from the last attempt only.
   - The source that passed (`target/e2e/<run>/source.txt`) and the attempt count.
   - Bugs you fixed along the way, and bugs outside this change, with screenshot paths.
   - Anything left unproven, and why.
   - The key screenshot for each criterion, and that the VMs are deleted.

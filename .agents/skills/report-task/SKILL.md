---
name: report-task
description: Turn a bug, feature or chore the user reports into a .plans/tasks file for /tasks-start, after confirming it against the code and reproducing it when needed. Use only when the user runs /report-task.
argument-hint: "<what you saw or want>"
disable-model-invocation: true
---

# Report task

Report: `$ARGUMENTS`. If it's empty, ask the user what they saw or want and
wait for the answer.

Repository root: `/Users/saeed/Projects/muxy`. Task folder: `.plans/tasks`
(call it `$T`). Task files use the format in
`.agents/skills/tasks-start/SKILL.md`. Read that section before writing one.

The user is here. The result is a task file that a `/tasks-start` round can
finish alone, or no file because the report isn't a real problem.

## Rules

- Write only task files in `$T`. Don't change code, commit, push, or touch
  GitHub issues and PRs.
- Don't switch branches or touch the user's uncommitted changes. Rounds build
  from `main`, so run `git fetch` and judge the report against `origin/main`
  (`git show origin/main:<path>`, `git grep <pattern> origin/main`).
- Reproduce in a detached worktree in the scratchpad
  (`git worktree add --detach <dir> origin/main`). Remove it with
  `git worktree remove --force <dir>` before you finish, and delete any VMs you
  created.
- The report is the user's own, so trust it, but confirm it before writing it
  up as fact.

## Steps

1. **Understand.** Read `docs/product/README.md`, `docs/tech/README.md` and
   the task files in `$T`. Restate the report in one or two sentences: what the
   user did, what happened and what they expected, or for a feature, what they
   want to do.
2. **Check it's new.** Look for the same thing in `$T`, in open issues
   (`gh issue list --state open --search '<keywords>'`) and in recent history
   on `origin/main`. If a task covers it, offer to add the new details to that
   task instead. If it's already fixed, name the commit or PR and stop.
   Then apply `tasks-start`'s "Task size": a new file is the exception. If it fits an
   open task, plan to add it there as a new part.
3. **Find it in the code.** For a bug, trace the code path and find the cause.
   For a feature, confirm it's missing and find where it would go. Name files
   and functions, not line numbers.
4. **Reproduce when the code doesn't settle it.** Use the cheapest proof that
   works:
   - a failing test in the worktree
   - the CLI or server run from the worktree
   - a VM for UI, window or platform behaviour, as `tasks-start`'s
     "Testing in VMs" describes. Export the worktree, not this checkout:
     `.agents/skills/e2e-test/scripts/e2e prepare <run> <worktree>`

   Skip this when the code already proves it, or for a feature. If you can't
   reproduce it, say what you tried and ask the user for what's missing
   (version, platform, exact steps, logs) before going on.
5. **Verdict.** Tell the user whether it's confirmed, not an issue (works as
   the docs describe, already fixed, or a setup problem) or unconfirmed, with
   the evidence. Propose the type, size, its place in the order, and which open
   task it joins, if any. If it isn't an issue, stop without a file unless the user
   still wants one.
6. **Clarify.** Ask everything a round would otherwise have to ask: the list in
   `tasks-start` Triage step 3, starting at "List everything an agent would
   otherwise have to ask". Include the proposed place in the order, or the task it
   joins, if the user might disagree. Use `AskUserQuestion`, up to 4 questions per call, with options
   and a recommendation. Anything the user defers goes under `## Questions`.
7. **Write** it. To join an open task, add a `###` part under its Problem, Changes
   and Tests. Otherwise write a new task file:
   - Name: `<number>-<slug>.md`, numbered as `tasks-start` describes.
   - **Problem:** in your own words, with the evidence: version, platform, the
     steps, what the reproduction showed and the cause in the code.
   - **Changes:** what to change. For a feature or a user-visible bug, add what
     the user sees and does when it's done. That's the scenario the round
     tests against.
   - **Tests:** turn the reproduction into the test that proves the fix.
   - **Decisions:** the user's answers as short instructions.
   - **Ready:** `yes` once it's confirmed and every question is answered,
     otherwise `no`.
   - Never split a task, and never make it depend on another one.
8. **Report.** Give the file path, title, type, number, size, ready state and a
   one-line problem. Say if it joined an existing task. List anything left open.

## Order

Most urgent first:

1. Crash, data loss, security hole, or something that blocks normal use.
2. A user-visible bug or missing behaviour that hurts, with a workaround.
3. Minor bugs, polish, small features, and refactors that pay off soon.
4. Large or optional work.

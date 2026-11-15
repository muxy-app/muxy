---
name: code-and-architecture-improvement
description: Make one small refactor that preserves behaviour, only when the user explicitly asks to run this skill. Discussing, reviewing or editing the skill does not run its workflow.
disable-model-invocation: true
---

Address one concrete maintenance problem with one refactor, as small as a
proper fix allows. Keep new or rewritten logic under about 300 lines. Tests,
code that only moves and call sites updated to follow it do not count. Preserve
observable behaviour, errors, public interfaces, and persisted/protocol formats.
No bug fixes, features or behaviour changes.

Make a proper structural fix, not a patch. No shims, pass-through wrappers,
flags, special cases or lint suppressions. Tests may move with the code, but
never weaken, skip or delete them to get checks passing. For a flaky test, one
that also fails intermittently on the starting commit, rerun it and name it in
the PR.

This skill runs unattended and on its own. Run no other skills. Never ask the
user or wait for approval. Where you would ask, go ahead if the question is
about the refactor. Otherwise choose the option that touches nothing outside
this run.

## Steps

1. Stop if the working tree has uncommitted changes. Otherwise switch to main
   and run git pull.
2. Read docs/tech/architecture.md, then find at least 3 candidate problems that
   fit the scope. Prefer structure over polish: misplaced responsibilities,
   leaky boundaries, duplicated logic. Weigh each by maintenance payoff, risk
   to behaviour and existing test coverage. Skip rename-only or formatting-only
   churn, areas touched by open PRs, refactors a closed `refactor/` PR already
   tried, and code whose behaviour this machine's checks cannot prove, such as
   rendering and input handling. Pick the strongest and explain why it helps.
   If none is worthwhile, stop without changes.
3. Create a `refactor/` branch and record the starting commit.
4. Add any missing tests for the behaviour the refactor touches and confirm
   they pass on the unchanged code. Then make the refactor.
5. Run this platform's `scripts/check-*.py` in full and fix what it finds.
6. Review once. Have a sub-agent read every changed line since the starting
   commit against AGENTS.md, unchanged behaviour and the scope limits, and
   report only problems the diff introduces, each with file:line and a
   concrete failure. Verify each finding, fix the real ones and rerun the
   checks. Do not review again.
7. Commit, push and open a pull request. Keep the description short: what the
   refactor improves and which checks passed.
8. Wait for CI and any review-bot comments. Apply the comments that are right
   and keep behaviour unchanged, then push. Squash-merge only after every CI
   job passes on the final commit and the change still fits the scope limits.
   Then delete the remote branch and return to main.

If checks cannot pass or the change outgrows the scope, close the PR if one
exists, delete the branch, return to main and report why. Do not fix bugs found
along the way. List them in the PR or the report.

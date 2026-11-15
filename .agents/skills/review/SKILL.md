---
name: review
description: Exhaustively review every line in a diff from a fixed point along two independent axes—repository standards and the originating spec—with evidenced findings, explicit coverage, and bounded fix verification.
---

# Code Review

Review one immutable diff against:

- **Standards** — repository instructions and documented coding standards.
- **Spec** — the originating issue, PRD, or acceptance criteria.

## Prepare

1. Pin the requested input. Resolve committed endpoints to SHAs; if staged or working-tree changes are included, capture one patch snapshot. Give every reviewer that same immutable diff. Stop on invalid or empty input.
2. Find the spec from commit references, a user-provided path, or matching files under `docs/`, `specs/`, or `.scratch/`. If none exists, mark the Spec axis unavailable.
3. Find applicable repository instructions and standards such as `CLAUDE.md`, `AGENTS.md`, `CONTRIBUTING.md`, and `CODING_STANDARDS.md`.
4. Inventory the complete diff: files, hunks, additions, deletions, renames, and binary files. Never rely on one possibly truncated diff output.
5. Find the specs of the changes. what feature it delivers, or what issue it solves. Use PR summary or linked issues or user provided context. If not sure, Ask user before proceeding. Sometimes its possible to understand the spec from the code changes as well.

## Review

- Inspect every added, changed, and deleted line in every file, including tests, docs, configuration, migrations, lock/generated files, renames, and binaries. Read surrounding code and call sites where needed.
- Maintain a file-and-hunk checklist against the inventory. Inspect file by file or in smaller batches so output cannot hide lines through truncation. Do not finish with an unchecked line or hunk.
- Report only issues introduced by the diff. A blocker requires evidence of an acceptance, correctness, security, data-loss, compatibility, or explicit required-standard violation. Style preferences, generic code smells, speculative hardening, optional refactors, and unrelated debt are non-blocking advisories.
- For findings, provide severity, `file:line`, the violated spec/rule or invariant, a concrete failure scenario, evidence, and the smallest valid fix. Do not report vague possibilities.
- Return `PASS` or `CHANGES NEEDED`, then blockers, advisories, and coverage totals. `PASS` means zero blockers, not zero possible improvements.

Compare both coverage reports with the inventory. If anything is missing, review only the missing batches before reporting; an incomplete review cannot pass.

## Report

- Present `## Standards` and `## Spec` separately. End with blocker and advisory counts per axis. Do not let advisories change a pass into failure.
- Also when presenting, each findings should have severity, High, Medium, Low and clarify which ones must be fixed before proceeding.
- Recommend `FIX BEFORE PROCEEDING`, `CONSIDER FIXING`, or `PROCEED` from the evidenced severity, confidence, and risk, and name the findings that drive the recommendation. The review is read-only: the user decides whether to fix. Ask for that decision and do not edit or dispatch a fixer without explicit approval.
- Do not report advisories unless user asks for them.
- Provide a TL;DR section in the end in short in a table


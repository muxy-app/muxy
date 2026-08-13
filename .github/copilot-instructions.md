# Code Review

Review every pull request for specification compliance and applicable repository standards using one evidence-gathering pass. Do not build, lint, run tests, or modify code during a review unless the user explicitly asks.

For changes of at most 5 files and 200 added or deleted lines, fetch the metadata and diff together, then inspect the changed code, direct references, and closest relevant tests in one context pass. Use one focused follow-up only when that evidence exposes a concrete ambiguity or risk. Expand proportionally for larger changes.

Stop when every changed behavior has been mapped to the specification, its direct interactions and relevant tests have been checked, and no concrete unresolved risk remains. State unavailable evidence instead of continuing open-ended exploration or guessing.

## Specification Compliance

- Understand the purpose of the changes from linked issues, the pull request title and description, commit messages, and the changed code.
- Verify that the changes fully achieve that purpose and preserve expected behavior.
- Do not invent requirements or make assumptions when evidence is unavailable.

## Repository Standards

- Read `AGENTS.md` and any instructions applicable to the changed files. `AGENTS.md` links to `CLAUDE.md`, so do not read both copies.
- Inspect the surrounding implementation and tests to understand established patterns.
- Apply security, maintainability, architecture, resource usage, concurrency, testing, documentation, and extension requirements only where they are relevant to the changed behavior.
- Identify root-cause problems rather than superficial symptoms.

## Findings

- Report only actionable defects supported by evidence and introduced by the pull request.
- Explain the concrete impact of each finding and identify the relevant changed code.
- Do not proactively search for unrelated problems. Report one separately only if it is encountered while following a changed code path.
- Do not modify code or apply review recommendations during a review.

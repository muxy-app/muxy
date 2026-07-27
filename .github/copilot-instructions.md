# Code Review

Review every pull request in two independent directions: specification compliance and repository standards. Analyze them separately so conclusions from one review do not bias the other.

## Specification Compliance

- Understand the purpose of the changes from linked issues, the pull request title and description, commit messages, and the changed code.
- Verify that the changes fully achieve that purpose and preserve expected behavior.
- Do not invent requirements or make assumptions when evidence is unavailable.

## Repository Standards

- Read `AGENTS.md`, `CLAUDE.md`, and any instructions applicable to the changed files.
- Inspect the surrounding implementation and tests to understand established patterns.
- Verify compliance with the repository's security, maintainability, scalability, clean architecture, resource usage, testing, documentation, and extension requirements.
- Identify root-cause problems rather than superficial symptoms.

## Findings

- Report only actionable defects supported by evidence and introduced by the pull request.
- Explain the concrete impact of each finding and identify the relevant changed code.
- Report pre-existing or unrelated problems separately from pull request findings.
- Do not modify code or apply review recommendations during a review.

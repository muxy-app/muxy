# Muxy

Requires macOS 14+ and Swift 6.0+. No external dependency managers needed — everything is SPM-based.

## Linting & Formatting

Requires `swiftlint` and `swiftformat` (`brew install swiftlint swiftformat`).

```bash
scripts/checks.sh             # Format, lint, build, test
scripts/checks.sh --fix       # Auto-fix formatting and linting issues
scripts/checks.sh --coverage  # Also run the coverage gate (slower; opt-in)
swiftformat --lint .          # Check formatting only
swiftlint lint --strict       # Check linting only
```

For code-changing tasks, run `scripts/checks.sh --fix` before handoff. Do not run it for read-only review or analysis unless the user explicitly asks.

Test processes use isolated Application Support storage.

## Top Level Rules

- Security first
- Maintainability
- Scalability
- Clean Code
- Clean Architecture
- Best Practices
- No Hacky Solutions
- Do not present assumptions as facts. Verify material claims and state uncertainty when evidence is unavailable.

## Main Rules

- No commenting allowed in the codebase
- All code must be self-explanatory and cleanly structured
- Use early returns instead of nested conditionals
- Don't patch symptoms, fix root causes
- For code changes, consider architecture and code quality in proportion to the change's scope and risk
- Follow existing code patterns. Keep optional refactors separate and within the requested scope.
- Use logs for debugging.
- Test the critical and reasonable paths only and do not overtest.
- Investigate the code directly relevant to the task. Expand beyond direct dependencies only when evidence identifies a concrete reason.
- Prioritize problem comprehension over premature implementation. Validate the approach before execution to avoid rework
- Plan in proportion to the task's complexity and risk
- Low memory and CPU usage is one of the key factors
- Simpler, flexible and scalable approaches are key factors
- Never run the app. User will run and test visually
- When code changes affect documented behavior, public APIs, hooks, configuration, or workflows, update the related documentation accurately.
- If contributed using AI, the LLM name is mandatory to be mentioned in the PR description.

## Extensions

- When providing API or hook or features to extensions, Make sure we update the extension SKILL and docs.
- Extension features usually need testing, offer a demo extension at ~/.config/muxy/extensions to the user.
- Prefix the demo extensions with `demo-*`

## Code Review

- Review changes against the stated PR, issue, or task. Start with the diff, directly affected code, and closest relevant tests. Expand only when a concrete ambiguity or risk requires it.
- Do not proactively search for unrelated issues. If one is encountered while following a changed code path, report it separately.
- Read-only reviews do not run builds, linters, tests, or `scripts/checks.sh` unless the user explicitly asks.
- Apply review recommendations only after user's confirmation.

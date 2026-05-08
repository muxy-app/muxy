# Muxy

Requires macOS 14+ and Swift 6.0+. No external dependency managers needed — everything is SPM-based.

## Linting & Formatting

Requires `swiftlint` and `swiftformat` (`brew install swiftlint swiftformat`).

```bash
scripts/checks.sh        # Run all checks (formatting, linting, build)
scripts/checks.sh --fix  # Auto-fix formatting and linting issues
swiftformat --lint .      # Check formatting only
swiftlint lint --strict   # Check linting only
```

Run `scripts/checks.sh --fix` after every task.

## Top Level Rules

- Security first
- Native Only
- Maintainability
- Scalability
- Clean Code
- Clean Architecture
- Best Practices
- No Hacky Solutions

## Main Rules

- No commenting allowed in the codebase
- All code must be self-explanatory and cleanly structured
- Use early returns instead of nested conditionals
- Don't patch symptoms, fix root causes
- For every task, Consider how it will impact the architecture and code quality, not just the immediate problem
- Follow the existing code's pattern but offer refactors if they improve code quality and maintainability.
- Use logs for debugging.
- If the feature is testable, then you must write tests.
- Avoid long PR descriptions. It is for humans and keep it in 3 lines maximum.
- Upload screenshots or recordings for the PRs.
- Never answer any question without a proper investigation and exploring the codebase.
- Prioritize problem comprehension over premature implementation. Validate the approach before execution to avoid rework
- Plan properly before executing to not double work

## Code Review

- Review the PRs/Code against the purpose of the PR/Issue/Asked. If you find unrelated issues to the PR during the review, Report them in a separate section.
- Apply review recommendations only after user's confirmation.

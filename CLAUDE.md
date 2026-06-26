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

Run `scripts/checks.sh --fix` after every task.

## Top Level Rules

- Security first
- Maintainability
- Scalability
- Clean Code
- Clean Architecture
- Best Practices
- No Hacky Solutions
- No guessing and No assumption! Work with certainity.

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
- Low memory and CPU usage is one of the key factors
- Simpler, flexible and scalable approaches are key factors
- Never run the app. User will run and test visually

## Extensions

- When providing API or hook or features to extensions, Make sure we update the extension SKILL and docs.
- Extension features usually need testing, offer a demo extension at ~/.config/muxy/extensions to the user.
- Prefix the demo extensions with `demo-*`

## Code Review

- Review the PRs/Code against the purpose of the PR/Issue/Asked. If you find unrelated issues to the PR during the review, Report them in a separate section.
- Apply review recommendations only after user's confirmation.

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:7510c1e2 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/SYNC_CONCEPTS.md for details and anti-patterns.

## Session Completion

**When ending a work session**, you MUST complete ALL steps below. Work is NOT complete until `git push` succeeds.

**MANDATORY WORKFLOW:**

1. **File issues for remaining work** - Create issues for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **PUSH TO REMOTE** - This is MANDATORY:
   ```bash
   git pull --rebase
   git push
   git status  # MUST show "up to date with origin"
   ```
5. **Clean up** - Clear stashes, prune remote branches
6. **Verify** - All changes committed AND pushed
7. **Hand off** - Provide context for next session

**CRITICAL RULES:**
- Work is NOT complete until `git push` succeeds
- NEVER stop before pushing - that leaves work stranded locally
- NEVER say "ready to push when you are" - YOU must push
- If push fails, resolve and retry until it succeeds
<!-- END BEADS INTEGRATION -->

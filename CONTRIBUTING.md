# Contributing to Muxy

Thank you for your interest in contributing to Muxy! This guide will help you get started.

## Getting Started

### Prerequisites

- macOS 14+
- Swift 6.0+
- [SwiftLint](https://github.com/realm/SwiftLint) and [SwiftFormat](https://github.com/nicklockwood/SwiftFormat) (`brew install swiftlint swiftformat`)

### Setup

```bash
git clone https://github.com/muxy-app/muxy.git
cd muxy
scripts/setup.sh          # downloads GhosttyKit.xcframework
swift build               # verify everything compiles
```

### Running

```bash
swift run Muxy
```

## Development Workflow

1. Fork the repository and create a branch from `main`
2. Make your changes
3. Run checks before committing:

```bash
scripts/checks.sh --fix   # auto-fix formatting and linting, then build
```

4. Push your branch and open a pull request

## Code Standards

- **No comments in the codebase** — all code must be self-explanatory and cleanly structured
- **Early returns** over nested conditionals
- **Fix root causes**, not symptoms
- **Follow existing patterns** but suggest refactors if they improve quality
- **Security first** — no command injection, XSS, or other vulnerabilities

## Checks

All PRs must pass these checks:

```bash
swiftformat --lint .       # formatting
swiftlint lint --strict    # linting
swift build                # compilation
```

Run `scripts/checks.sh` to execute all three at once, or `scripts/checks.sh --fix` to auto-fix what can be fixed.

## Pull Request Guidelines

- Keep PRs focused on a single change
- Write a clear title and description explaining the "why"
- Ensure all checks pass before requesting review
- Link any related issues

## Reporting Issues

- Use the [Bug Report](.github/ISSUE_TEMPLATE/bug_report.yml) template for bugs
- Use the [Feature Request](.github/ISSUE_TEMPLATE/feature_request.yml) template for ideas
- Search existing issues before creating a new one

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).

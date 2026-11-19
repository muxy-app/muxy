# Contributing to Muxy

Thank you for your interest in contributing to Muxy! Bug reports, ideas, and
pull requests are all welcome.

## Reporting Bugs

Search [existing issues](https://github.com/muxy-app/muxy/issues) first. If
yours is already there, add your details to it instead of opening a new one.

A good bug report includes:

- **Versions:** the output of `muxy --build-info` and `muxy server status`
- **System:** macOS, Linux, or Windows, and its version
- **Steps to reproduce:** the smallest set of steps that shows the problem
- **Expected and actual behavior:** what you thought would happen, and what
  happened instead
- **Logs:** the relevant lines from the logs listed in
  [Troubleshooting](docs/user-guide/troubleshooting.md#logs)
- **Screenshots or recordings**, when the problem is visual

The more details you share, the faster a bug can be fixed. Remove anything
private, such as paths, hostnames, or tokens, before posting logs.

## Suggesting Ideas

Open an issue that describes:

- **The problem:** what you are trying to do, and what gets in the way
- **Your idea:** how you imagine Muxy could solve it
- **Alternatives:** other ways you have tried, or seen in other tools

Explaining the problem matters more than the solution. It helps find the
approach that fits Muxy best.

## Getting Started

Read [docs/product](docs/product/README.md) and [docs/tech](docs/tech/README.md) first. They describe how Muxy works and how it is built.

### Prerequisites

- macOS 14+ for the desktop app, or Linux for the CLI and server
- [Rust](https://rustup.rs) (`rustup` installs the version pinned in `rust-toolchain.toml`)
- [Zig](https://ziglang.org) 0.17.0, used to build the Ghostty terminal library

### Setup

```bash
git clone https://github.com/muxy-app/muxy.git
cd muxy
```

On macOS, build the whole workspace with `cargo build --workspace`. On Linux,
build the CLI and server with `cargo build -p muxy-cli -p muxy-server`; the
desktop app is macOS only.

### Running

```bash
cargo run -p muxy-app     # desktop app (macOS)
cargo run -p muxy-cli     # muxy terminal UI
```

Build as in [Setup](#setup) first, so `muxy-server` sits next to the binary you
run. Development builds keep their data in a separate `Muxy Dev` profile
(`muxy-dev` on Linux).

## Development Workflow

1. Fork the repository and create a branch from `main`
2. Make your changes
3. Run checks before committing (on Linux, see [Checks](#checks)):

```bash
cargo fmt --all
cargo xcheck              # clippy with warnings as errors
cargo xtest               # all tests
```

4. Commit your changes, push your branch, and open a pull request yourself

## Code Standards

- **No comments in the codebase** — all code must be self-explanatory and cleanly structured
- **Early returns** over nested conditionals
- **Fix root causes**, not symptoms
- **Follow existing patterns** but suggest refactors if they improve quality
- **Security first** — no command injection, XSS, or other vulnerabilities

## Checks

All PRs must pass CI, which runs on macOS, Linux, and Windows. Run its main
checks locally:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
```

On Linux, CI runs two checks. `scripts/check-headless.py` covers the CLI, the
server, and the crates they depend on. It needs glibc 2.35; elsewhere, run Clippy
and tests with `-p` for each package that
`python3 scripts/check-headless.py --packages` lists, instead of `--workspace`.
`scripts/check-desktop-linux.py` covers the whole workspace, including the
experimental desktop app, on Ubuntu 24.04.

A pull request that changes only source files in `crates/muxy-app` or
`crates/muxy-ui` tests just those two crates and skips the headless check. Any
other change, including a `Cargo.toml` or `Cargo.lock`, and every push to `main`
run everything.

## Pull Request Guidelines

- Keep PRs focused on a single change
- Write a clear title and description yourself, in your own words, explaining the "why"
- Ensure all checks pass before requesting review
- Link any related issues
- If you used AI to help write code, name the LLM in your own PR description

## Extensions

Extensions live in their own repository. See
[Contributing an Extension](docs/extensions/contributing.md).

## License

By contributing, you agree that your contributions will be licensed under the [Apache License 2.0](LICENSE).

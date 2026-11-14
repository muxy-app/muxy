# Contributing to Muxy

Thank you for your interest in Muxy!

Muxy doesn't accept pull requests. All code will be written by its maintainer. The
best way to contribute is to open an issue: a clear bug report or a well
described idea helps more than a patch.

## Reporting Bugs

Search [existing issues](https://github.com/muxy-app/muxy/issues) first. If
yours is already there, add your details to it instead of opening a new one.

A good bug report includes:

- **Versions:** the output of `muxy --build-info` and `muxy server status`
- **System:** macOS or Linux, and its version
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

## Extensions

Extensions live in their own repository and still accept pull requests. See
[Contributing an Extension](docs/extensions/contributing.md).

## Building from Source

You can build Muxy yourself to try the latest changes on `main` or to check
whether a bug is already fixed. Read [docs/product](docs/product/README.md) and
[docs/tech](docs/tech/README.md) to learn how Muxy works and how it is built.

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

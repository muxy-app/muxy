# Muxy

A terminal multiplexer in Rust. `muxy-server` owns the terminals and projects.
The desktop app, the `muxy` CLI and terminal UI, and the phone apps reach it
only through the protocol. `docs/product/` and `docs/tech/` describe the design.

## Ground rules

- Code comments are not allowed anywhere in the project.
- Tests must bring value. Test the critical path and real risks: data loss,
  security, compatibility, concurrency, regressions. A test must fail when the
  behaviour it covers is removed. Do not test literals, defaults or UI details
  such as a button's title, a colour or a layout.

## Good to know

Facts that are hard to see from the code. They are not rules: use judgment
about what a change needs.

- Builds of different versions talk to each other. `docs/tech/protocol.md` has
  the compatibility rules.
- Some pages under `docs/` are compiled into the app as reference for its
  agents: `user-guide/settings.md`, `user-guide/keyboard-shortcuts.md`,
  `features/ai-agents.md`, `features/muxy-cli.md` and everything in
  `extensions/`.
- CI builds and tests on macOS, Linux and Windows. A local run covers one.
- CI runs the `scripts/check-*.py` scripts: Clippy with warnings as errors,
  then the workspace tests. A test that runs over 60 seconds fails the run.
- Only the Linux job checks formatting, so the macOS script does not cover
  `cargo fmt --all`.
- `cargo xcheck` and `cargo xtest` are aliases for the workspace Clippy and
  test runs.

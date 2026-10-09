# Agents

## Repository

- Before finishing: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings`.

## Main Guides

- Don't run the app for visual testing. All visual testings must be done by user.
- Only test real scenarios and risks (data loss, security, compatibility, concurrency, regressions). Don't test obvious code, defaults, or UI details like pixels, layout, colors, and titles.

## Third-party dependencies

When adding new third-party dependencies, provide user with their github url and stars and metrics if they are being maintained so user can approve adding it.

## Performance

- Do not ignore performance and memory efficiency. with GPUI it is easy to cause high CPU and memory usage.

## Only Humans

AI agents are not allowed to do the followings even if a human asks:

- Commit and push
- Open a PR
- Open an issue
- Interact with issues or PRs

These all have to be done by humans with their own language without any help from AI.

Any PRs or issues opened by AI will be closed without checking.
# Vendored crates

These crates are upstream copies with local changes. The root `Cargo.toml`
swaps them in through `[patch.crates-io]`.

Each crate keeps its full diff against upstream in
`<crate>/patches/upstream-*.patch`. Apply it with `git apply` inside a fresh
upstream copy to rebuild the vendored crate. Packaging files (`LICENSE`,
`Cargo.toml.orig`, `.cargo_vcs_info.json`) and upstream files we don't build
(README, tests, binaries) are left out of the diff.

| Crate | Upstream | Why it's vendored | Changes |
| --- | --- | --- | --- |
| `core-text` | crates.io 21.0.0 | gpui and font-kit need 21.x, and 21.0.0 leaks font descriptor arrays | `CTFontCollection::get_descriptors` takes ownership under the Create rule ([upstream fix](https://github.com/servo/core-foundation-rs/commit/6f844cf1a1a18e25b70fcdf1bcdc458555bd2eff)). `CTRun` getters spell out their elided `Cow` lifetimes. |
| `xim-ctext` | crates.io 0.4.1, kept at version 0.3.0 | `zed-xim`, behind gpui's X11 input method, asks for 0.3, which can't decode Chinese, Korean, ISO-8859 or mixed-charset compound text | 0.4.1's decoder under the 0.3.0 version number. Text before the first escape is decoded as plain text, so commits like `(June)` or `$(echo)` aren't mistaken for escape sequences ([upstream PR](https://github.com/Riey/xim-rs/pull/50)). The demo binary is dropped. |
| `libghostty-vt` | [libghostty-rs `9ede35b8`](https://github.com/uzaaft/libghostty-rs/commit/9ede35b8ea94a55ed3608408d67e102883129f1e), unreleased | Muxy needs a newer Ghostty and a clear-screen call that no release has | `Terminal::clear_screen` wraps the patched Ghostty call. `vt_write_until_ground` uses Ghostty's newer `vt_write_until`. `key::Encoder::encode_to_vec` reserves the full required length when it retries. |
| `libghostty-vt-sys` | [libghostty-rs `9ede35b8`](https://github.com/uzaaft/libghostty-rs/commit/9ede35b8ea94a55ed3608408d67e102883129f1e), unreleased | Builds the Ghostty that `libghostty-vt` needs | Pins Ghostty `83fb9562`. Applies `patches/clear-screen.patch` to a private copy of the Ghostty source before building (`patch.rs`). Retries Ghostty and Zig package downloads. Hides the app's Git tags from Ghostty's build. Rejects a pkg-config Ghostty that lacks the patch. `src/bindings.rs` is regenerated for that Ghostty. |

`Cargo.toml` in both libghostty crates is Cargo's packaged form of upstream's
manifest, without the `readme` line. Upstream's manifest itself is kept
unchanged as `Cargo.toml.orig`. `build.rs.orig` is upstream's build script.

`libghostty-vt-sys/patches/clear-screen.patch` is different from the
`upstream-*.patch` files: it changes Ghostty's own source, and the build
applies it.

## gpui

gpui comes from the [muxy-app/gpui](https://github.com/muxy-app/gpui) fork,
pinned by revision in the root `Cargo.toml`
(`ec24331257b2565cff99ad09a99d5f5855af9be0`). The fork starts from gpui 0.2.2
on crates.io. Muxy relies on these changes on top of it:

- Keep presenting frames only during high-rate input.
- Use memoryless storage for the path MSAA texture on Apple GPUs.
- Fix ownership of oversized images in the Metal atlas.
- Size Metal frame buffers to the current rendering work.
- Release path textures when frames stop drawing paths.
- Read copied files from the Linux clipboard.
- Give X11 windows their raw handles.
- Release the window lock before resigning a spurious key window on macOS.
- Composite translucent layers with "over" alpha on macOS.
- Truncate text from its full runs, and measure it again once its width is
  known.

`git log` in the fork lists each change.

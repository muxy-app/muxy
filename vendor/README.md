# Vendored crates

These crates are upstream copies with local changes. The root `Cargo.toml`
swaps them in through `[patch.crates-io]`.

Each crate keeps its full diff against upstream in
`<crate>/patches/upstream-*.patch`. Apply it with `git apply` inside a fresh
upstream copy, outside this repository, to rebuild the vendored crate. Packaging files (`LICENSE`,
`Cargo.toml.orig`, `.cargo_vcs_info.json`) and upstream files we don't build
(README, tests, binaries) are left out of the diff.

| Crate | Upstream | Why it's vendored | Changes |
| --- | --- | --- | --- |
| `core-text` | crates.io 21.0.0 | gpui and font-kit need 21.x, and 21.0.0 leaks font descriptor arrays | `CTFontCollection::get_descriptors` takes ownership under the Create rule ([upstream fix](https://github.com/servo/core-foundation-rs/commit/6f844cf1a1a18e25b70fcdf1bcdc458555bd2eff)). `CTRun` getters spell out their elided `Cow` lifetimes. |
| `xim-ctext` | crates.io 0.4.1, kept at version 0.3.0 | `zed-xim`, behind gpui's X11 input method, asks for 0.3, which can't decode Chinese, Korean, ISO-8859 or mixed-charset compound text | 0.4.1's decoder ([upstream PR](https://github.com/Riey/xim-rs/pull/50)) under the 0.3.0 version number. Local fix: text before the first escape is decoded as plain text, so commits like `(June)` or `$(echo)` aren't mistaken for escape sequences. Upstream only guards `(B` and `(J` so far. The demo binary is dropped. |
| `libghostty-vt` | [libghostty-rs `9ede35b8`](https://github.com/uzaaft/libghostty-rs/commit/9ede35b8ea94a55ed3608408d67e102883129f1e), unreleased | Muxy needs a newer Ghostty, and a clear-screen call and access to every frame of an animated image that no release has | `Terminal::clear_screen` wraps the patched Ghostty call. In `kitty::graphics`, `Image::frame`, `frame_count`, `animation_state`, `current_frame` and `loops` wrap the patched Ghostty calls that read every stored frame of an animated image and its playback state. `vt_write_until_ground` uses Ghostty's newer `vt_write_until`. `key::Encoder::encode_to_vec` reserves the full required length when it retries. `snapshot::Decoder::set_kitty_image_storage_limit` sets Ghostty's image limit for a restored terminal, which a snapshot doesn't carry. |
| `libghostty-vt-sys` | [libghostty-rs `9ede35b8`](https://github.com/uzaaft/libghostty-rs/commit/9ede35b8ea94a55ed3608408d67e102883129f1e), unreleased | Builds the Ghostty that `libghostty-vt` needs | Pins Ghostty `83fb9562`. Applies `patches/clear-screen.patch`, `patches/kitty-image-file.patch` and `patches/kitty-animation-frames.patch` to a private copy of the Ghostty source before building (`patch.rs`). Retries Ghostty and Zig package downloads. Hides the app's Git tags from Ghostty's build. Rejects a pkg-config Ghostty that lacks the clear-screen or the animation-frames patch. Keeps Zig's build outputs in `LIBGHOSTTY_VT_SYS_ZIG_CACHE_DIR` when it's set, so CI can reuse them. `src/bindings.rs` is regenerated for that Ghostty. |

`Cargo.toml` in both libghostty crates is Cargo's packaged form of upstream's
manifest, without the `readme` line. Upstream's manifest itself is kept
unchanged as `Cargo.toml.orig`. `build.rs.orig` is upstream's build script.

`libghostty-vt-sys/patches/clear-screen.patch`,
`libghostty-vt-sys/patches/kitty-image-file.patch` and
`libghostty-vt-sys/patches/kitty-animation-frames.patch` are different from
the `upstream-*.patch` files: they change Ghostty's own source, and the build
applies them in that order. The Kitty image patch stats an image path before
opening it and opens files to read without waiting, so a FIFO can't stall the
terminal. It also gives libghostty's small I/O layer the path stat it needs on
POSIX. The Kitty animation patch adds read-only C API calls: Ghostty stores
every frame of an animated image but only hands out the one on screen, and
these return any stored frame with its gap, the frame count, the playback
state, the current frame and the loop count. Muxy's server sends the frames to
the app, which plays them itself. A pkg-config Ghostty needs all three
patches.

## gpui

gpui is gpui 0.2.2 from crates.io with Muxy's changes, edited here directly.
It came from the [muxy-app/gpui](https://github.com/muxy-app/gpui) fork,
copied at commit `35acfec`. The fork is no longer updated; its `git log`
explains each change made before the copy. It has diverged too far from
upstream to rebase onto, so it keeps no `upstream-*.patch`. `Cargo.lock` is
left out, and an empty `[workspace]` table in its `Cargo.toml` keeps it out of
Muxy's workspace, so `cargo test` runs its own tests from `vendor/gpui`.
`Cargo.toml` is edited by hand; `Cargo.toml.orig` and `.cargo_vcs_info.json`
are upstream's packaging files, kept for reference only.

Muxy relies on these changes:

- Keep presenting frames only during high-rate input.
- Use memoryless storage for the path MSAA texture on Apple GPUs.
- Fix ownership of oversized images in the Metal atlas.
- Size Metal frame buffers to the current rendering work.
- Release path textures when frames stop drawing paths.
- Keep atlas textures alive until the GPU is done with them.
- Read copied files from the Linux clipboard.
- Give X11 windows their raw handles.
- Release the window lock before resigning a spurious key window on macOS.
- Composite translucent layers with "over" alpha on macOS.
- Truncate text from its full runs, and measure it again once its width is
  known.
- Publish an accessibility tree through AccessKit, and roll its nodes back
  with a failed prepaint.
- Move a window's traffic lights after it opens on macOS.
- Accept file URL and path text drops, and keep panels from becoming the main
  window on macOS.
- Hand an app the keystroke gpui would dispatch for a native key event.
- Hide a pressed element's tooltip until the mouse leaves it.
- Place an IME composition caret from the Wayland preedit cursor and the X11
  preedit caret, counted in UTF-16 units of the composing text, and anchor the
  candidate window to that caret.

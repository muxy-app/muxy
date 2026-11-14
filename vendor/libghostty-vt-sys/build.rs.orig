use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned ghostty commit. Update this to pull a newer version.
const GHOSTTY_REPO: &str = "https://github.com/ghostty-org/ghostty.git";
const GHOSTTY_COMMIT: &str = "521cf2c5d30bd48028ca61546c64e7913764073d";

/// File name of the static archive on Windows. Ghostty installs it under this
/// name for every Windows ABI so it does not collide with `ghostty-vt.lib`,
/// the import library for `ghostty-vt.dll`. Validation and link emission must
/// agree on it, or the build silently links the import library instead.
const WINDOWS_STATIC_LIB_FILE: &str = "ghostty-vt-static.lib";

#[derive(Clone, Copy)]
enum LinkMode {
    Dynamic,
    Static,
}

impl LinkMode {
    fn current() -> Self {
        if cfg!(feature = "link-dynamic") {
            Self::Dynamic
        } else {
            Self::Static
        }
    }

    fn artifact_kind(self) -> &'static str {
        match self {
            Self::Dynamic => "shared library",
            Self::Static => "static library",
        }
    }

    fn matches_library(self, target: &str, file_name: &str) -> bool {
        match self {
            Self::Dynamic => {
                if target.contains("darwin") {
                    file_name.starts_with("libghostty-vt") && file_name.ends_with(".dylib")
                } else if target.contains("windows") {
                    file_name == "ghostty-vt.lib"
                        || file_name == "ghostty-vt.dll"
                        || file_name == "libghostty-vt.dll.lib"
                        || file_name == "libghostty-vt.dll.a"
                } else {
                    file_name == "libghostty-vt.so" || file_name.starts_with("libghostty-vt.so.")
                }
            }
            Self::Static => {
                if target.contains("windows") {
                    file_name == WINDOWS_STATIC_LIB_FILE
                } else {
                    file_name == "libghostty-vt.a"
                }
            }
        }
    }

    #[cfg(feature = "pkg-config")]
    fn pkg_config_name(self) -> &'static str {
        match self {
            Self::Dynamic => "libghostty-vt",
            Self::Static => "libghostty-vt-static",
        }
    }
}

fn main() {
    // docs.rs has no Zig toolchain. The checked-in bindings in src/bindings.rs
    // are enough for generating documentation, so skip the entire native
    // build when running under docs.rs.
    if env::var("DOCS_RS").is_ok() {
        return;
    }

    // Miri cannot load or call the native Ghostty library. The Miri suite stays
    // within Rust-owned seams, so there is nothing to build or link for it.
    if env::var("CARGO_CFG_MIRI").is_ok() {
        return;
    }

    let link_mode = LinkMode::current();
    // Cargo always sets TARGET for build scripts, so read it once here and
    // hand it to whichever path runs rather than re-reading it per call site.
    let target = env::var("TARGET").expect("TARGET must be set");

    println!("cargo:rerun-if-env-changed=LIBGHOSTTY_VT_SYS_CPU");
    println!("cargo:rerun-if-env-changed=LIBGHOSTTY_VT_SYS_OPTIMIZE");
    println!("cargo:rerun-if-env-changed=GHOSTTY_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=GHOSTTY_ZIG_SYSTEM_DIR");
    println!("cargo:rerun-if-env-changed=ZIG");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=HOST");
    println!("cargo:rerun-if-env-changed=DEBUG");
    println!("cargo:rerun-if-env-changed=OPT_LEVEL");
    println!("cargo:rerun-if-changed=crates/libghostty-vt-sys/build.rs");

    // An explicit source override should stay authoritative even when the
    // pkg-config feature is enabled, so local Ghostty checkouts remain easy to
    // test against.
    if env::var_os("GHOSTTY_SOURCE_DIR").is_some() {
        build_vendored(link_mode, &target);
        return;
    }

    // When the pkg-config feature is enabled, prefer an installed library over
    // fetching Ghostty. libghostty is pre-1.0, so this crate intentionally does
    // not promise compatibility with every installed C API revision.
    #[cfg(feature = "pkg-config")]
    if try_pkg_config(link_mode, &target) {
        return;
    }

    build_vendored(link_mode, &target);
}

/// Build libghostty-vt from source via zig. The zig build itself generates
/// shared and static artifacts plus pkg-config files in `share/pkgconfig/`.
fn build_vendored(link_mode: LinkMode, target: &str) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR must be set"));
    let host = env::var("HOST").expect("HOST must be set");

    // Locate ghostty source: env override > fetch into OUT_DIR.
    let ghostty_dir = match env::var("GHOSTTY_SOURCE_DIR") {
        Ok(dir) => {
            let p = PathBuf::from(dir);
            assert!(
                p.join("build.zig").exists(),
                "GHOSTTY_SOURCE_DIR does not contain build.zig: {}",
                p.display()
            );
            p
        }
        Err(_) => fetch_ghostty(&out_dir),
    };

    // Build libghostty-vt via zig.
    let install_prefix = out_dir.join("ghostty-install");
    // Tell the gen-bindings tool, a binary of this crate, which headers
    // this exact build installed. Cargo keeps one build output per feature
    // set and profile, and ones from before a pin bump still hold the old
    // headers, so scanning the target directory can find stale ones.
    println!(
        "cargo:rustc-env=LIBGHOSTTY_VT_SYS_INCLUDE_DIR={}",
        install_prefix.join("include").display()
    );
    let zig_cache_dir = out_dir.join("zig-cache");
    let zig_global_cache_dir = out_dir.join("zig-global-cache");

    let optimize = zig_optimize_mode();
    let cpu = env::var("LIBGHOSTTY_VT_SYS_CPU").unwrap_or_else(|_| "baseline".to_owned());
    assert!(
        !cpu.is_empty(),
        "LIBGHOSTTY_VT_SYS_CPU must not be empty when set"
    );

    // iOS builds go through ghostty's emit-xcframework path instead of a flat
    // `-Dtarget=<ios> --sysroot=<sdk>` invocation. The flat build breaks down
    // for iOS: a generic target baseline can't compile simdutf's always_inline
    // NEON intrinsics, and `--sysroot` applies globally so it leaks into the
    // native codegen tools ghostty runs mid-build. The xcframework path runs
    // host-native and configures each Apple platform itself, so we build that
    // and pull out the library we need afterwards.
    let ios_platform = ios_xcframework_platform(target);
    if ios_platform.is_some() {
        // Ghostty only emits the xcframework when zig itself runs on macOS
        // (it shells out to xcodebuild). Without this check a Linux cross
        // build would silently produce a host-native library and then fail on
        // a confusing missing-xcframework assertion after the full build.
        assert!(
            host.contains("apple-darwin"),
            "building for {target} requires a macOS host with Xcode and the iOS SDK \
             (ghostty's emit-xcframework path runs host-native); host is {host}"
        );
        // The xcframework contains only static archives, and the flat layout
        // below is populated from one of them. Fail up front instead of
        // letting the shared-library search fail with a misleading message.
        assert!(
            matches!(link_mode, LinkMode::Static),
            "building for {target} supports static linking only; \
             disable the link-dynamic feature"
        );
    }

    // Users can override the zig command with the `ZIG` env variable.
    // We default to assuming zig is in path.
    let zig = env::var_os("ZIG").unwrap_or_else(|| "zig".into());
    let mut build = Command::new(zig);
    build
        .arg("build")
        .arg("-Demit-lib-vt=true")
        .arg(format!("-Doptimize={optimize}"))
        // Cargo artifacts may run on older CPUs than the build host. Without
        // an explicit CPU model, Zig may emit host-specific instructions that
        // make distributed binaries fail with an illegal instruction. Users
        // building for a known machine can explicitly request `native` or a
        // named Zig CPU model through LIBGHOSTTY_VT_SYS_CPU.
        //
        // For iOS builds this only affects the host-native flat artifacts:
        // ghostty resolves its own per-platform targets for the xcframework
        // slices, so -Dcpu does not leak into them.
        .arg(format!("-Dcpu={cpu}"))
        .arg(if ios_platform.is_some() {
            "-Demit-xcframework=true"
        } else {
            "-Demit-xcframework=false"
        })
        .arg("-Dapp-runtime=none")
        .arg("--prefix")
        .arg(&install_prefix)
        .arg("--cache-dir")
        .arg(&zig_cache_dir)
        .current_dir(&ghostty_dir);

    // Package managers can provide Ghostty's Zig package cache ahead of time
    // and ask Zig to resolve packages from that immutable store path instead
    // of fetching during this Cargo build script. Those sandboxes usually
    // have no writable home either, so the global cache goes under OUT_DIR.
    // Zig 0.17 removed `--global-cache-dir`; the global cache can only be
    // redirected through the environment now (as ghostty's own
    // nix/build-support/update-mirror.nu does).
    if let Ok(dir) = env::var("GHOSTTY_ZIG_SYSTEM_DIR") {
        assert!(
            !dir.is_empty(),
            "GHOSTTY_ZIG_SYSTEM_DIR must not be empty when set"
        );
        let zig_system_dir = PathBuf::from(dir);
        assert!(
            zig_system_dir.exists(),
            "GHOSTTY_ZIG_SYSTEM_DIR does not exist: {}",
            zig_system_dir.display()
        );
        build
            .arg("--system")
            .arg(&zig_system_dir)
            .env("ZIG_GLOBAL_CACHE_DIR", &zig_global_cache_dir);
    }

    // Pass -Dtarget only for non-iOS cross targets; native builds let zig
    // auto-detect the host. iOS builds run host-native inside the xcframework
    // emit, so they must not pass -Dtarget or a global --sysroot.
    if target != host && ios_platform.is_none() {
        let zig_target = zig_target(target);
        build.arg(format!("-Dtarget={zig_target}"));
    }

    run(build, "zig build");

    // The emit also installs host-native flat artifacts; replace them with the
    // iOS library so the link emission below picks up the right arch.
    if let Some(platform) = ios_platform {
        extract_xcframework_lib(&install_prefix, platform);
    }

    let lib_dir = install_prefix.join("lib");
    let include_dir = install_prefix.join("include");
    let search_dirs = library_search_dirs(target, &install_prefix);
    if ios_platform.is_none() {
        warn_unused_xcframework(&lib_dir);
    }

    let has_requested_library = search_dirs.iter().any(|dir| {
        std::fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
            .any(|entry| {
                let entry = entry.unwrap_or_else(|error| {
                    panic!("failed to read entry from {}: {error}", dir.display())
                });
                let file_name = entry.file_name();
                let Some(file_name) = file_name.to_str() else {
                    return false;
                };

                link_mode.matches_library(target, file_name)
            })
    });
    assert!(
        has_requested_library,
        "expected libghostty-vt {} in one of {:?}",
        link_mode.artifact_kind(),
        search_dirs
    );
    assert!(
        include_dir.join("ghostty").join("vt.h").exists(),
        "expected header at {}",
        include_dir.join("ghostty").join("vt.h").display()
    );

    for dir in &search_dirs {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    match link_mode {
        LinkMode::Dynamic => println!("cargo:rustc-link-lib=dylib=ghostty-vt"),
        LinkMode::Static => emit_static_link_lib(target),
    }
    emit_include_metadata(&[include_dir]);
}

/// Emit the link directive for the static archive.
///
/// Zig names static archives `<name>.lib` on every Windows ABI, so neither
/// name rustc derives from a plain `static=ghostty-vt` finds the archive
/// there: MSVC resolves it to `ghostty-vt.lib`, the DLL import library, which
/// links but leaves a load-time dependency on `ghostty-vt.dll`; the GNU
/// targets look for `libghostty-vt.a`, which no Windows build produces, and
/// fail outright. Link the exact file name Ghostty installs instead, which
/// `+verbatim` passes through to rustc's archive lookup untouched.
fn emit_static_link_lib(target: &str) {
    if target.contains("windows") {
        println!("cargo:rustc-link-lib=static:+verbatim={WINDOWS_STATIC_LIB_FILE}");
    } else {
        println!("cargo:rustc-link-lib=static=ghostty-vt");
    }
}

fn warn_unused_xcframework(lib_dir: &Path) {
    let xcframework = lib_dir.join("ghostty-vt.xcframework");
    if xcframework.exists() {
        println!(
            "cargo:warning=unused libghostty-vt XCFramework emitted at {}; Cargo links the dylib or archive directly",
            xcframework.display()
        );
    }
}

#[cfg(feature = "pkg-config")]
fn try_pkg_config(link_mode: LinkMode, target: &str) -> bool {
    let mut config = pkg_config::Config::new();
    let lib = match link_mode {
        LinkMode::Dynamic => config.probe(link_mode.pkg_config_name()),
        LinkMode::Static => config
            .statik(true)
            .cargo_metadata(false)
            .probe(link_mode.pkg_config_name()),
    };
    let lib = match lib {
        Ok(lib) => lib,
        Err(_) => return false,
    };

    if let LinkMode::Static = link_mode {
        emit_static_pkg_config_metadata(&lib, target);
    }
    emit_include_metadata(&lib.include_paths);
    true
}

#[cfg(feature = "pkg-config")]
fn emit_static_pkg_config_metadata(lib: &pkg_config::Library, target: &str) {
    for path in &lib.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    for path in &lib.link_files {
        if let Some(parent) = path.parent() {
            println!("cargo:rustc-link-search=native={}", parent.display());
        }
    }
    for path in &lib.framework_paths {
        println!("cargo:rustc-link-search=framework={}", path.display());
    }
    for framework in &lib.frameworks {
        println!("cargo:rustc-link-lib=framework={framework}");
    }

    emit_static_link_lib(target);
    for library in &lib.libs {
        if library != "ghostty-vt" {
            println!("cargo:rustc-link-lib={library}");
        }
    }
    for args in &lib.ld_args {
        if !args.is_empty() {
            println!("cargo:rustc-link-arg=-Wl,{}", args.join(","));
        }
    }
}

fn emit_include_metadata(include_paths: &[PathBuf]) {
    if include_paths.is_empty() {
        return;
    }

    let joined = env::join_paths(include_paths)
        .unwrap_or_else(|error| panic!("failed to join include paths for cargo metadata: {error}"));
    println!("cargo:include={}", joined.to_string_lossy());
}

/// Decide which Zig `OptimizeMode` to pass to `zig build`.
///
/// The `LIBGHOSTTY_VT_SYS_OPTIMIZE` environment variable overrides this unconditionally; accepted
/// values are the four Zig `OptimizeMode` names (`Debug`, `ReleaseSafe`, `ReleaseFast`,
/// `ReleaseSmall`).
///
/// Defaults to `ReleaseFast` for optimized builds. If `DEBUG` is `true` (as cargo sets for the
/// `dev` profile), `Debug` mode is used. Otherwise, if `OPT_LEVEL` is `s` or `z`, `ReleaseSmall`
/// is used.
fn zig_optimize_mode() -> &'static str {
    if let Ok(override_mode) = env::var("LIBGHOSTTY_VT_SYS_OPTIMIZE") {
        return match override_mode.as_str() {
            "Debug" => "Debug",
            "ReleaseSafe" => "ReleaseSafe",
            "ReleaseFast" => "ReleaseFast",
            "ReleaseSmall" => "ReleaseSmall",
            other => panic!(
                "LIBGHOSTTY_VT_SYS_OPTIMIZE must be one of Debug, ReleaseSafe, ReleaseFast, ReleaseSmall (got '{other}')"
            ),
        };
    }

    if env::var("DEBUG").as_deref() == Ok("true") {
        return "Debug";
    }

    match env::var("OPT_LEVEL").as_deref() {
        Ok("s") | Ok("z") => "ReleaseSmall",
        _ => "ReleaseFast",
    }
}

/// Clone ghostty at the pinned commit into OUT_DIR/ghostty-src.
/// Reuses an existing clone if the commit matches.
fn fetch_ghostty(out_dir: &Path) -> PathBuf {
    let src_dir = out_dir.join("ghostty-src");
    let stamp = src_dir.join(".ghostty-commit");

    // Skip fetch if we already have the right commit.
    if stamp.exists()
        && let Ok(existing) = std::fs::read_to_string(&stamp)
        && existing.trim() == GHOSTTY_COMMIT
    {
        return src_dir;
    }

    // Clean and clone fresh.
    if src_dir.exists() {
        std::fs::remove_dir_all(&src_dir)
            .unwrap_or_else(|e| panic!("failed to remove {}: {e}", src_dir.display()));
    }

    eprintln!("Fetching ghostty {GHOSTTY_COMMIT} ...");

    let mut clone = Command::new("git");
    // Cargo's nested OUT_DIR plus Ghostty's fuzz corpus names can exceed
    // Windows MAX_PATH. Scope long-path support to this fetched repository.
    clone
        .arg("clone")
        .arg("--config")
        .arg("core.longpaths=true")
        .arg("--filter=blob:none")
        .arg("--no-checkout")
        .arg(GHOSTTY_REPO)
        .arg(&src_dir);
    run(clone, "git clone ghostty");

    let mut checkout = Command::new("git");
    checkout
        .arg("checkout")
        .arg(GHOSTTY_COMMIT)
        .current_dir(&src_dir);
    run(checkout, "git checkout ghostty commit");

    std::fs::write(&stamp, GHOSTTY_COMMIT).unwrap_or_else(|e| panic!("failed to write stamp: {e}"));

    src_dir
}

fn run(mut command: Command, context: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to execute {context}: {error}"));
    assert!(status.success(), "{context} failed with status {status}");
}

/// Returns directories to search for the built library artifact.
/// On Windows, Zig may place the DLL in `bin/` and the import lib in `lib/`,
/// so both are included.
fn library_search_dirs(target: &str, install_prefix: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![install_prefix.join("lib")];
    if target.contains("windows") {
        dirs.push(install_prefix.join("bin"));
    }
    dirs
}

/// The platform directory inside `ghostty-vt.xcframework` for an iOS Rust
/// target, or `None` for targets that build directly via `-Dtarget`. Ghostty
/// emits an arm64-only simulator library (what the simulator runs on Apple
/// silicon), so x86_64-apple-ios is not supported here.
fn ios_xcframework_platform(target: &str) -> Option<&'static str> {
    match target {
        "aarch64-apple-ios" => Some("ios-arm64"),
        "aarch64-apple-ios-sim" => Some("ios-arm64-simulator"),
        _ => None,
    }
}

/// Copy an xcframework platform's static lib + headers into the flat
/// `<prefix>/lib/libghostty-vt.a` and `<prefix>/include` layout the link
/// emission expects, replacing the host-native artifacts the emit installed.
fn extract_xcframework_lib(install_prefix: &Path, platform: &str) {
    let platform_dir = install_prefix
        .join("lib")
        .join("ghostty-vt.xcframework")
        .join(platform);
    // Ghostty emits iOS xcframework slices only when it detects the iOS SDK,
    // silently skipping them otherwise, so a missing directory here almost
    // always means the SDK is absent rather than a build failure.
    assert!(
        platform_dir.is_dir(),
        "expected xcframework platform dir {platform} at {}; \
         ghostty emits iOS slices only when the iOS SDK is detected, \
         so install it via Xcode (Settings > Components)",
        platform_dir.display()
    );
    // ghostty names the Apple static libraries `libghostty-vt-fat.a`.
    let src_lib = platform_dir.join("libghostty-vt-fat.a");
    assert!(
        src_lib.exists(),
        "expected static lib at {}",
        src_lib.display()
    );

    let lib_dir = install_prefix.join("lib");
    let dest_lib = lib_dir.join("libghostty-vt.a");
    std::fs::copy(&src_lib, &dest_lib).unwrap_or_else(|error| {
        panic!(
            "failed to copy {} -> {}: {error}",
            src_lib.display(),
            dest_lib.display()
        )
    });

    // Headers are arch-independent, but prefer the platform's own copy.
    let headers = platform_dir.join("Headers");
    if headers.is_dir() {
        copy_dir_all(&headers, &install_prefix.join("include"));
    }
}

/// Recursively copy a directory tree (merging into `dst` if it exists).
fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", dst.display()));
    let entries = std::fs::read_dir(src)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", src.display()));
    for entry in entries {
        let entry = entry
            .unwrap_or_else(|error| panic!("failed to read entry in {}: {error}", src.display()));
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry
            .file_type()
            .unwrap_or_else(|error| panic!("failed to stat {}: {error}", from.display()));
        if file_type.is_dir() {
            copy_dir_all(&from, &to);
        } else {
            std::fs::copy(&from, &to)
                .unwrap_or_else(|error| panic!("failed to copy {}: {error}", from.display()));
        }
    }
}

fn zig_target(target: &str) -> String {
    let value = match target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
        "aarch64-apple-darwin" => "aarch64-macos-none",
        "x86_64-apple-darwin" => "x86_64-macos-none",
        "x86_64-pc-windows-gnu" => "x86_64-windows-gnu",
        "aarch64-pc-windows-gnullvm" => "aarch64-windows-gnu",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        "aarch64-linux-android" => "aarch64-linux-android",
        "x86_64-linux-android" => "x86_64-linux-android",
        other => panic!("unsupported Rust target for vendored build: {other}"),
    };
    value.to_owned()
}

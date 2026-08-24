use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SLICE_NAME: &str = "macos-arm64_x86_64";
const SETUP_HINT: &str = "run ./scripts/setup.sh from the muxy-native repository root";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GHOSTTY_XCFRAMEWORK_PATH");
    println!("cargo:rerun-if-env-changed=SDKROOT");
    println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");

    validate_target();

    let framework = xcframework_path();
    let slice = framework.join(SLICE_NAME);
    let header = slice.join("Headers/ghostty.h");
    let archive = slice.join("ghostty-internal.a");
    require_file(&header, "Ghostty C header");
    require_file(&archive, "Ghostty static library");
    println!("cargo:rerun-if-changed={}", header.display());
    println!("cargo:rerun-if-changed={}", archive.display());

    let sdk = macos_sdk_path();
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must provide OUT_DIR"));
    prepare_link_archive(&archive, &out_dir);
    emit_linker_configuration(&out_dir);
    generate_bindings(&header, &sdk, &out_dir);
}

fn validate_target() {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if os != "macos" || !matches!(arch.as_str(), "aarch64" | "x86_64") {
        panic!("ghostty-sys supports only macOS arm64 and x86_64 targets; got {arch}-{os}");
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("ghostty-sys must live under crates/")
        .to_path_buf()
}

fn xcframework_path() -> PathBuf {
    env::var_os("GHOSTTY_XCFRAMEWORK_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| repository_root().join("vendor/GhosttyKit.xcframework"))
}

fn require_file(path: &Path, description: &str) {
    if !path.is_file() {
        panic!(
            "missing {description} at {}. Set GHOSTTY_XCFRAMEWORK_PATH to a valid GhosttyKit.xcframework or {SETUP_HINT}",
            path.display()
        );
    }
}

fn macos_sdk_path() -> PathBuf {
    if let Some(sdkroot) = env::var_os("SDKROOT").filter(|value| !value.is_empty()) {
        let path = PathBuf::from(sdkroot);
        if path.is_dir() {
            return path;
        }
    }

    let output = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to execute xcrun while locating the macOS SDK: {error}. Install and select full Xcode, then {SETUP_HINT}"
            )
        });
    if !output.status.success() {
        panic!(
            "xcrun could not locate the macOS SDK: {}. Install full Xcode and select it with `sudo xcode-select -s /Applications/Xcode.app/Contents/Developer`, then {SETUP_HINT}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let path = PathBuf::from(OsString::from(
        String::from_utf8(output.stdout)
            .expect("xcrun SDK path must be UTF-8")
            .trim(),
    ));
    if !path.is_dir() {
        panic!(
            "xcrun returned a missing macOS SDK path at {}. Select full Xcode, then {SETUP_HINT}",
            path.display()
        );
    }
    path
}

fn prepare_link_archive(archive: &Path, out_dir: &Path) {
    let link_name = out_dir.join("libghostty-internal.a");
    if link_name.symlink_metadata().is_ok() {
        fs::remove_file(&link_name)
            .unwrap_or_else(|error| panic!("failed to replace {}: {error}", link_name.display()));
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(archive, &link_name).unwrap_or_else(|error| {
        panic!(
            "failed to create linker alias {} -> {}: {error}",
            link_name.display(),
            archive.display()
        )
    });

    #[cfg(not(unix))]
    compile_error!("ghostty-sys build support requires a Unix host");
}

fn emit_linker_configuration(out_dir: &Path) {
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=ghostty-internal");

    for framework in [
        "AppKit",
        "AVFoundation",
        "Carbon",
        "CoreAudio",
        "CoreGraphics",
        "CoreText",
        "Foundation",
        "IOKit",
        "IOSurface",
        "CoreVideo",
        "Metal",
        "MetalKit",
        "QuartzCore",
        "Speech",
        "UserNotifications",
    ] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    println!("cargo:rustc-link-lib=dylib=c++");
    println!("cargo:rustc-link-lib=dylib=sqlite3");
}

fn generate_bindings(header: &Path, sdk: &Path, out_dir: &Path) {
    let bindings = bindgen::Builder::default()
        .header(header.to_string_lossy())
        .clang_arg("-isysroot")
        .clang_arg(sdk.to_string_lossy())
        .clang_arg("-mmacosx-version-min=14.0")
        .allowlist_item("ghostty_.*")
        .allowlist_item("GHOSTTY_.*")
        .derive_default(true)
        .generate_comments(true)
        .layout_tests(false)
        .generate()
        .unwrap_or_else(|error| {
            panic!(
                "bindgen failed for {} using SDK {}: {error}. Ensure full Xcode and its command-line tools are installed",
                header.display(),
                sdk.display()
            )
        });

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write generated Ghostty bindings");
}

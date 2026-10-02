use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn prepare(source: &Path, out_dir: &Path) -> PathBuf {
    let patched = out_dir.join("ghostty-patched");
    if patched.exists() {
        fs::remove_dir_all(&patched).expect("remove previous private Ghostty copy");
    }
    copy_tree(source, &patched);
    let patch = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("patches/clear-screen.patch");
    let status = Command::new("git")
        .current_dir(&patched)
        .env("GIT_CEILING_DIRECTORIES", out_dir)
        .arg("apply")
        .arg(patch)
        .status()
        .expect("apply native Ghostty clear API patch");
    assert!(status.success(), "native Ghostty clear API patch failed");
    patched
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("create private Ghostty source directory");
    for entry in fs::read_dir(source).expect("read Ghostty source directory") {
        let entry = entry.expect("read Ghostty source entry");
        if matches!(
            entry.file_name().to_str(),
            Some(".git" | ".zig-cache" | "zig-out")
        ) {
            continue;
        }
        let target = destination.join(entry.file_name());
        if entry
            .file_type()
            .expect("read Ghostty source type")
            .is_dir()
        {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy Ghostty source file");
        }
    }
}

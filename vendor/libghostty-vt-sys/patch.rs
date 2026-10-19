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
    let patches = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("patches");
    for name in ["clear-screen.patch", "grapheme-cluster.patch"] {
        let status = Command::new("git")
            .current_dir(&patched)
            .env("GIT_CEILING_DIRECTORIES", out_dir)
            .arg("apply")
            .arg(patches.join(name))
            .status()
            .expect("apply native Ghostty patch");
        assert!(status.success(), "native Ghostty patch {name} failed");
    }
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
        let file_type = entry.file_type().expect("read Ghostty source type");
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target);
        } else if !file_type.is_symlink() || links_to_file(&entry.path()) {
            fs::copy(entry.path(), target).expect("copy Ghostty source file");
        }
    }
}

/// Symlinks are copied as the file they point to. Links to directories and
/// broken links are skipped, since Windows can't always create or follow them.
fn links_to_file(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

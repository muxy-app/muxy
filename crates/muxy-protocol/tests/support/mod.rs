use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::{env, fs, process};

pub(crate) struct TempFolder {
    pub(crate) path: PathBuf,
}

impl TempFolder {
    pub(crate) fn new() -> io::Result<Self> {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let path = env::temp_dir().join(format!(
            "muxy-t-{}-{}",
            process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        #[cfg_attr(windows, allow(unused_mut))]
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder.create(&path)?;
        Ok(Self { path })
    }

    pub(crate) fn socket(&self) -> PathBuf {
        self.path.join("socket")
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[allow(dead_code)]
pub(crate) fn run_test_in_child(test: &str, variable: &str, value: &str) -> io::Result<Child> {
    Command::new(env::current_exe()?)
        .args(["--exact", test, "--nocapture"])
        .env(variable, value)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
}

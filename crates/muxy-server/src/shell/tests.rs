use super::*;

use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn install_rejects_symlinked_directories_and_replaces_files_without_following_links() -> TestResult
{
    let directory = std::env::temp_dir().join(format!(
        "muxy-shell-install-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory)?;
    let target = directory.join("target");
    fs::write(&target, "untouched")?;
    let hooks = directory.join("hooks");
    ShellIntegration::install(&hooks)?;
    fs::remove_file(hooks.join("muxy.zsh"))?;
    std::os::unix::fs::symlink(&target, hooks.join("muxy.zsh"))?;
    ShellIntegration::install(&hooks)?;
    assert_eq!(fs::read_to_string(&target)?, "untouched");
    let linked = directory.join("linked");
    std::os::unix::fs::symlink(&hooks, &linked)?;
    assert!(ShellIntegration::install(&linked).is_err());
    fs::remove_dir_all(&directory)?;
    Ok(())
}

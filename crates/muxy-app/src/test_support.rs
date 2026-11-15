use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;

thread_local! {
    static PROJECT: OnceCell<tempfile::TempDir> = const { OnceCell::new() };
    static DIRECTORIES: RefCell<Vec<tempfile::TempDir>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn project_directory() -> PathBuf {
    PROJECT.with(|project| project.get_or_init(new).path().to_path_buf())
}

pub(crate) fn directory() -> PathBuf {
    let directory = new();
    let path = directory.path().to_path_buf();
    DIRECTORIES.with_borrow_mut(|directories| directories.push(directory));
    path
}

fn new() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("muxy-app-test-")
        .tempdir()
        .expect("a temporary test directory")
}

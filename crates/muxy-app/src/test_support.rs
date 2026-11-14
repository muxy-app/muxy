//! Directories for tests that are removed when the test ends.
//!
//! Each test runs on its own thread, so directories held by the thread are
//! dropped, and deleted, as soon as the test returns or panics.

use std::cell::{OnceCell, RefCell};
use std::path::PathBuf;

thread_local! {
    static PROJECT: OnceCell<tempfile::TempDir> = const { OnceCell::new() };
    static DIRECTORIES: RefCell<Vec<tempfile::TempDir>> = const { RefCell::new(Vec::new()) };
}

/// An empty folder for projects to point at, the same one throughout a test.
///
/// Unlike the system temporary directory, it holds nothing that scanning a
/// project, such as for its icon, could find or spend time on.
pub(crate) fn project_directory() -> PathBuf {
    PROJECT.with(|project| project.get_or_init(new).path().to_path_buf())
}

/// A new empty folder, removed when the test ends.
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

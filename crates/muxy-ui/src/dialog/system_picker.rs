//! File and folder pickers through the system's own dialog, which GPUI opens.

use std::io;
use std::path::{Path, PathBuf};

use gpui::{App, PathPromptOptions};

/// An open picker. Dropping it leaves the dialog open, but its answer is
/// never delivered.
#[derive(Debug)]
pub struct FolderPicker {
    _open: async_channel::Sender<()>,
}

pub fn choose_folder(
    cx: &mut App,
    message: &str,
    directory: &Path,
    on_complete: impl FnOnce(io::Result<Option<PathBuf>>) + 'static,
) -> io::Result<FolderPicker> {
    choose_folder_with(cx, message, "", directory, on_complete)
}

/// A folder picker whose confirm button reads `prompt`, when it is not empty.
pub fn choose_folder_with(
    cx: &mut App,
    _message: &str,
    prompt: &str,
    _directory: &Path,
    on_complete: impl FnOnce(io::Result<Option<PathBuf>>) + 'static,
) -> io::Result<FolderPicker> {
    Ok(open(cx, false, prompt, on_complete))
}

pub fn choose_file(
    cx: &mut App,
    _message: &str,
    _directory: &Path,
    on_complete: impl FnOnce(io::Result<Option<PathBuf>>) + 'static,
) -> io::Result<FolderPicker> {
    Ok(open(cx, true, "", on_complete))
}

fn open(
    cx: &mut App,
    files: bool,
    prompt: &str,
    on_complete: impl FnOnce(io::Result<Option<PathBuf>>) + 'static,
) -> FolderPicker {
    let answer = cx.prompt_for_paths(PathPromptOptions {
        files,
        directories: !files,
        multiple: false,
        prompt: (!prompt.is_empty()).then(|| prompt.to_owned().into()),
    });
    let (open, dropped) = async_channel::bounded::<()>(1);
    cx.spawn(async move |_| {
        let answer = match answer.await {
            Ok(Ok(paths)) => Ok(paths.and_then(only)),
            Ok(Err(error)) => Err(io::Error::other(error.to_string())),
            Err(_) => Ok(None),
        };
        if !dropped.is_closed() {
            on_complete(answer);
        }
    })
    .detach();
    FolderPicker { _open: open }
}

/// The one path a confirmed dialog chose. Anything else is a cancel.
fn only(paths: Vec<PathBuf>) -> Option<PathBuf> {
    let [path] = <[PathBuf; 1]>::try_from(paths).ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_chosen_path_is_a_choice() {
        assert_eq!(
            only(vec![PathBuf::from(r"C:\a")]),
            Some(PathBuf::from(r"C:\a"))
        );
        assert_eq!(only(Vec::new()), None);
        assert_eq!(
            only(vec![PathBuf::from(r"C:\a"), PathBuf::from(r"C:\b")]),
            None
        );
    }
}

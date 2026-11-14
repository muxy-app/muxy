//! Path comparison and the home folder, following each platform's rules.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

/// Windows file systems ignore case; Unix ones do not.
const IGNORE_CASE: bool = cfg!(windows);

/// Whether both paths have the same components, ignoring case on Windows.
pub fn path_eq(first: &Path, second: &Path) -> bool {
    components_eq(first, second, IGNORE_CASE)
}

/// Whether `path` is `base` or inside it, ignoring case on Windows.
pub fn path_starts_with(path: &Path, base: &Path) -> bool {
    starts_with(path, base, IGNORE_CASE)
}

/// `path` with a leading `home` written as `~`, ignoring case on Windows.
/// A path outside `home` is returned unchanged.
pub fn shorten_home(path: &Path, home: &Path) -> PathBuf {
    shorten_home_with(path, home, IGNORE_CASE)
}

fn shorten_home_with(path: &Path, home: &Path, ignore_case: bool) -> PathBuf {
    if home.as_os_str().is_empty() || !starts_with(path, home, ignore_case) {
        return path.to_owned();
    }
    let mut short = PathBuf::from("~");
    short.extend(path.components().skip(home.components().count()));
    short
}

fn components_eq(first: &Path, second: &Path, ignore_case: bool) -> bool {
    first.components().count() == second.components().count()
        && starts_with(first, second, ignore_case)
}

fn starts_with(path: &Path, base: &Path, ignore_case: bool) -> bool {
    let mut path = path.components();
    base.components().all(|base| {
        path.next().is_some_and(|component| {
            component_eq(component.as_os_str(), base.as_os_str(), ignore_case)
        })
    })
}

fn component_eq(first: &OsStr, second: &OsStr, ignore_case: bool) -> bool {
    match (first.to_str(), second.to_str()) {
        (Some(first), Some(second)) if ignore_case => {
            first.chars().map(upcase).eq(second.chars().map(upcase))
        }
        _ => first == second,
    }
}

/// NTFS upcases one character to one character, so `ß` never equals `SS`.
fn upcase(character: char) -> char {
    let mut upper = character.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(upper), None) => upper,
        _ => character,
    }
}

/// The user's home folder: `HOME` on Unix, the profile folder on Windows.
pub fn home_dir() -> io::Result<PathBuf> {
    std::env::home_dir()
        .filter(|home| !home.as_os_str().is_empty())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Home directory unavailable"))
}

/// Replaces a leading `~` component with the home folder. On Windows `~\`
/// works as well as `~/`. `~user` is left alone.
pub fn expand_tilde(path: &Path) -> io::Result<PathBuf> {
    expand_tilde_with(path, home_dir)
}

fn expand_tilde_with(
    path: &Path,
    home: impl FnOnce() -> io::Result<PathBuf>,
) -> io::Result<PathBuf> {
    match path.strip_prefix("~") {
        Ok(rest) if rest.as_os_str().is_empty() => home(),
        Ok(rest) => Ok(home()?.join(rest)),
        Err(_) => Ok(path.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_is_ignored_only_where_the_file_system_ignores_it() {
        let (lower, upper) = (
            Path::new("/users/me/Projekt-Ä"),
            Path::new("/Users/ME/projekt-ä"),
        );
        assert!(components_eq(lower, upper, true));
        assert!(!components_eq(lower, upper, false));
        assert_eq!(path_eq(lower, upper), cfg!(windows));
        assert!(path_eq(Path::new("/a//b/"), Path::new("/a/b")));
        assert!(!path_eq(Path::new("/a/b"), Path::new("/a/b/c")));
        assert!(!components_eq(Path::new("/a/b"), Path::new("/a/bc"), true));
        assert!(!components_eq(
            Path::new("/straße"),
            Path::new("/STRASSE"),
            true
        ));
        assert!(!components_eq(Path::new("/ﬁle"), Path::new("/FILE"), true));
        assert!(components_eq(
            Path::new("/straße"),
            Path::new("/STRAßE"),
            true
        ));
    }

    #[test]
    fn a_path_starts_with_whole_components_only() {
        let base = Path::new("/Users/me");
        assert!(path_starts_with(Path::new("/Users/me"), base));
        assert!(path_starts_with(Path::new("/Users/me/src"), base));
        assert!(!path_starts_with(Path::new("/Users/meow"), base));
        assert!(!path_starts_with(Path::new("/Users"), base));
        assert!(starts_with(Path::new("/USERS/ME/src"), base, true));
        assert!(!starts_with(Path::new("/USERS/ME/src"), base, false));
        assert_eq!(
            path_starts_with(Path::new("/USERS/ME/src"), base),
            cfg!(windows)
        );
    }

    #[test]
    fn only_paths_inside_the_home_folder_are_shortened() {
        let home = Path::new("/Users/me");
        let short = |path: &str, ignore_case| shorten_home_with(Path::new(path), home, ignore_case);
        assert_eq!(short("/Users/me", false), Path::new("~"));
        assert_eq!(short("/Users/me/", false), Path::new("~"));
        assert_eq!(short("/Users/me/src/muxy", false), Path::new("~/src/muxy"));
        assert_eq!(
            short("/Users/meow/src", false),
            Path::new("/Users/meow/src")
        );
        assert_eq!(short("/Users", false), Path::new("/Users"));
        assert_eq!(short("/USERS/ME/src", false), Path::new("/USERS/ME/src"));
        assert_eq!(short("/USERS/ME/src", true), Path::new("~/src"));
        assert_eq!(
            shorten_home(Path::new("/USERS/ME/src"), home),
            if cfg!(windows) {
                Path::new("~/src")
            } else {
                Path::new("/USERS/ME/src")
            }
        );
        assert_eq!(
            shorten_home(Path::new("/src"), Path::new("")),
            Path::new("/src")
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_windows_profile_folder_is_shortened_whatever_its_case() {
        let home = Path::new(r"C:\Users\Me");
        assert_eq!(
            shorten_home(Path::new(r"c:\users\me\src"), home).as_os_str(),
            r"~\src"
        );
        assert_eq!(
            shorten_home(Path::new(r"D:\Users\Me\src"), home),
            Path::new(r"D:\Users\Me\src")
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_drive_letters_and_separators_compare_equal() {
        assert!(path_eq(Path::new(r"C:\Users\Me"), Path::new("c:/users/me")));
        assert!(path_starts_with(
            Path::new(r"c:\users\me\src"),
            Path::new(r"C:\Users\Me")
        ));
        assert!(!path_eq(
            Path::new(r"C:\Users\Me"),
            Path::new(r"D:\Users\Me")
        ));
    }

    #[test]
    fn a_leading_tilde_component_becomes_the_home_folder() -> io::Result<()> {
        let home = PathBuf::from("/home/me");
        let expand = |path: &str| expand_tilde_with(Path::new(path), || Ok(home.clone()));
        assert_eq!(expand("~")?, home);
        assert_eq!(expand("~/")?, home);
        assert_eq!(expand("~/src/muxy")?, home.join("src/muxy"));
        assert_eq!(expand("~user/src")?, PathBuf::from("~user/src"));
        assert_eq!(expand("src/~")?, PathBuf::from("src/~"));
        assert_eq!(expand("/abs")?, PathBuf::from("/abs"));
        let backslash = expand(r"~\src")?;
        if cfg!(windows) {
            assert_eq!(backslash, home.join("src"));
        } else {
            assert_eq!(backslash, PathBuf::from(r"~\src"));
        }
        Ok(())
    }

    #[test]
    fn expanding_a_path_without_tilde_never_needs_a_home_folder() -> io::Result<()> {
        let missing = || Err(io::Error::from(io::ErrorKind::NotFound));
        assert_eq!(
            expand_tilde_with(Path::new("src"), missing)?,
            PathBuf::from("src")
        );
        assert!(expand_tilde_with(Path::new("~/src"), missing).is_err());
        Ok(())
    }

    #[test]
    fn the_home_folder_is_known() -> io::Result<()> {
        assert!(home_dir()?.is_absolute());
        Ok(())
    }
}

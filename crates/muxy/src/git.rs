use muxy_api::git::GitOptions;
use std::collections::HashMap;
use std::path::PathBuf;

const SEARCH_PATHS: [&str; 6] = [
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];

pub fn options() -> GitOptions {
    GitOptions {
        executable: PathBuf::from("git"),
        environment: HashMap::from([
            ("GIT_OPTIONAL_LOCKS".to_owned(), "0".to_owned()),
            ("PATH".to_owned(), search_path()),
        ]),
    }
}

fn search_path() -> String {
    let inherited = std::env::var("PATH").unwrap_or_default();
    let mut entries: Vec<&str> = inherited
        .split(':')
        .filter(|entry| !entry.is_empty())
        .collect();
    for candidate in SEARCH_PATHS {
        if !entries.contains(&candidate) {
            entries.push(candidate);
        }
    }
    entries.join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_path_appends_missing_defaults_once() {
        let value = search_path();
        let entries: Vec<&str> = value.split(':').collect();

        for candidate in SEARCH_PATHS {
            assert_eq!(
                entries.iter().filter(|entry| **entry == candidate).count(),
                1,
                "{candidate} should appear exactly once"
            );
        }
    }
}

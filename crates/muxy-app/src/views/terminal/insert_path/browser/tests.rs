use super::*;

fn entry(name: &str, is_directory: bool) -> Entry {
    Entry {
        name: name.to_owned(),
        is_directory,
    }
}

fn project() -> Vec<Entry> {
    vec![
        entry(".git", true),
        entry(".gitignore", false),
        entry("Cargo.lock", false),
        entry("Cargo.toml", false),
        entry("crates", true),
        entry("docs", true),
        entry("LICENSE", false),
        entry("locales", true),
    ]
}

fn browser(recents: &[&str]) -> Browser {
    Browser::new(
        Some("/Users/dev/Projects/muxy".to_owned()),
        Some("/Users/dev".to_owned()),
        recents.iter().map(|path| (*path).to_owned()).collect(),
    )
    .unwrap()
}

fn names(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|row| row.name.as_str()).collect()
}

#[test]
fn starts_in_the_terminals_folder_or_home_and_names_it_from_home() {
    let browser = browser(&[]);
    assert_eq!(browser.directory(), "/Users/dev/Projects/muxy");
    assert_eq!(browser.label(), "~/Projects/muxy/");

    let home = Browser::new(None, Some("/Users/dev/".to_owned()), Vec::new()).unwrap();
    assert_eq!(home.directory(), "/Users/dev");
    assert_eq!(home.label(), "~/");

    let elsewhere = Browser::new(Some("/etc".to_owned()), None, Vec::new()).unwrap();
    assert_eq!(elsewhere.label(), "/etc/");
    let root = Browser::new(Some("/".to_owned()), None, Vec::new()).unwrap();
    assert_eq!(root.label(), "/");
    assert!(Browser::new(None, None, Vec::new()).is_none());
}

#[test]
fn lists_names_in_order_and_hides_dot_names_until_the_filter_has_a_dot() {
    let browser = browser(&[]);
    let entries = project();
    assert_eq!(
        names(&browser.rows(&entries, "")),
        [
            "Cargo.lock",
            "Cargo.toml",
            "crates",
            "docs",
            "LICENSE",
            "locales"
        ]
    );
    assert_eq!(
        names(&browser.rows(&entries, ".")),
        [".git", ".gitignore", "Cargo.lock", "Cargo.toml"]
    );
    assert_eq!(names(&browser.rows(&entries, ".giti")), [".gitignore"]);
    assert!(Browser::hides(&entries));
    assert!(!Browser::hides(&[entry("docs", true)]));
}

#[test]
fn filters_ignoring_case_with_names_that_start_with_the_filter_first() {
    let browser = browser(&[]);
    let entries = project();
    assert_eq!(
        names(&browser.rows(&entries, "c")),
        [
            "Cargo.lock",
            "Cargo.toml",
            "crates",
            "docs",
            "LICENSE",
            "locales"
        ]
    );
    assert_eq!(
        names(&browser.rows(&entries, "LO")),
        ["locales", "Cargo.lock"]
    );
    assert!(browser.rows(&entries, "zzz").is_empty());
}

#[test]
fn recent_choices_and_the_folders_that_hold_them_come_first() {
    let browser = browser(&[
        "/Users/dev/Projects/muxy/docs/features/terminal.md",
        "/Users/dev/Projects/muxy/LICENSE",
        "/Users/dev/Projects/other/crates",
        "/Users/dev/Projects/muxy/docsite",
    ]);
    let rows = browser.rows(&project(), "");
    assert_eq!(
        names(&rows),
        [
            "docs",
            "LICENSE",
            "Cargo.lock",
            "Cargo.toml",
            "crates",
            "locales"
        ]
    );
    assert_eq!(
        rows.iter().map(|row| row.recent).collect::<Vec<_>>(),
        [true, true, false, false, false, false]
    );
    assert_eq!(
        names(&browser.rows(&project(), "l")),
        ["LICENSE", "locales", "Cargo.lock", "Cargo.toml"]
    );
}

#[test]
fn long_folders_list_only_their_first_rows() {
    let entries: Vec<_> = (0..ROW_LIMIT + 20)
        .map(|index| entry(&format!("file-{index:04}"), false))
        .collect();
    let browser = browser(&[]);
    assert_eq!(browser.rows(&entries, "").len(), ROW_LIMIT);
    assert_eq!(names(&browser.rows(&entries, "0519")), ["file-0519"]);
}

#[test]
fn typed_folders_are_followed_and_the_rest_filters() {
    let entries = project();
    let mut browser = browser(&[]);
    assert_eq!(browser.follow("doc", &entries), None);
    assert_eq!(browser.directory(), "/Users/dev/Projects/muxy");

    assert_eq!(browser.follow("docs/fea", &entries).as_deref(), Some("fea"));
    assert_eq!(browser.directory(), "/Users/dev/Projects/muxy/docs");

    assert_eq!(browser.follow("../", &[]).as_deref(), Some(""));
    assert_eq!(browser.directory(), "/Users/dev/Projects/muxy");

    assert_eq!(
        browser
            .follow("crates/muxy-app/./src//ma", &entries)
            .as_deref(),
        Some("ma")
    );
    assert_eq!(
        browser.directory(),
        "/Users/dev/Projects/muxy/crates/muxy-app/src"
    );

    assert_eq!(browser.follow("~/Doc", &[]).as_deref(), Some("Doc"));
    assert_eq!(browser.directory(), "/Users/dev");
    assert_eq!(browser.follow("/usr/lo", &[]).as_deref(), Some("lo"));
    assert_eq!(browser.directory(), "/usr");
    assert_eq!(browser.follow("../../", &[]).as_deref(), Some(""));
    assert_eq!(browser.directory(), "/");
}

#[test]
fn a_typed_folder_takes_the_spelling_the_folder_has() {
    let entries = vec![
        entry("Docs", false),
        entry("docs", true),
        entry("Crates", true),
    ];
    let mut exact = browser(&[]);
    exact.follow("docs/", &entries);
    assert!(exact.directory().ends_with("/docs"));
    let mut other_case = browser(&[]);
    other_case.follow("crates/Muxy-App/", &entries);
    assert!(
        other_case.directory().ends_with("/Crates/Muxy-App"),
        "only the listed folder's spelling is known"
    );
    let mut file = browser(&[]);
    file.follow("DOCS/", &[entry("Docs", false)]);
    assert!(file.directory().ends_with("/DOCS"), "a file is no folder");
}

#[test]
fn entering_and_leaving_folders_stops_at_the_root() {
    let mut browser = Browser::new(Some("/usr".to_owned()), None, Vec::new()).unwrap();
    browser.enter("local");
    assert_eq!(browser.directory(), "/usr/local");
    assert!(browser.leave());
    assert!(browser.leave());
    assert_eq!(browser.directory(), "/");
    assert!(!browser.leave());
    browser.enter("etc");
    assert_eq!(browser.directory(), "/etc");
}

#[test]
fn chosen_paths_are_relative_inside_the_terminals_folder() {
    let mut browser = browser(&[]);
    assert_eq!(
        browser.chosen("Cargo.toml", false),
        Chosen {
            absolute: "/Users/dev/Projects/muxy/Cargo.toml".to_owned(),
            typed: "Cargo.toml".to_owned(),
        }
    );
    assert_eq!(browser.chosen("docs", true).typed, "docs/");
    browser.enter("crates");
    assert_eq!(browser.chosen("muxy-app", true).typed, "crates/muxy-app/");
    browser.follow("~/", &[]);
    assert_eq!(
        browser.chosen(".ssh", true),
        Chosen {
            absolute: "/Users/dev/.ssh".to_owned(),
            typed: "/Users/dev/.ssh/".to_owned(),
        }
    );
    browser.follow("/Users/dev/Projects/", &[]);
    assert_eq!(
        browser.chosen("muxy-old", true).typed,
        "/Users/dev/Projects/muxy-old/",
        "a folder beside the terminal's is not inside it"
    );

    let unknown = Browser::new(None, Some("/Users/dev".to_owned()), Vec::new()).unwrap();
    assert_eq!(
        unknown.chosen("notes.txt", false).typed,
        "/Users/dev/notes.txt"
    );
    let root = Browser::new(Some("/".to_owned()), None, Vec::new()).unwrap();
    assert_eq!(root.chosen("etc", true).typed, "etc/");
}

#[test]
fn windows_paths_keep_their_separators_and_drives() {
    let mut browser = Browser::new(
        Some(r"C:\Users\dev\code".to_owned()),
        Some(r"C:\Users\dev".to_owned()),
        vec![r"C:\Users\dev\code\src\main.rs".to_owned()],
    )
    .unwrap();
    assert_eq!(browser.label(), r"~\code\");
    let rows = browser.rows(&[entry("build", true), entry("src", true)], "");
    assert_eq!(names(&rows), ["src", "build"]);
    assert_eq!(browser.chosen("src", true).typed, r"src\");

    assert_eq!(browser.follow("src/bin\\ma", &[]).as_deref(), Some("ma"));
    assert_eq!(browser.directory(), r"C:\Users\dev\code\src\bin");
    assert_eq!(browser.chosen("main.rs", false).typed, r"src\bin\main.rs");

    assert_eq!(browser.follow(r"D:\", &[]).as_deref(), Some(""));
    assert_eq!(browser.directory(), r"D:\");
    assert!(!browser.leave());
    assert_eq!(browser.label(), r"D:\");
    assert_eq!(browser.chosen("data", true).typed, r"D:\data\");

    let mut share = Browser::new(Some(r"\\host\share\a".to_owned()), None, Vec::new()).unwrap();
    assert!(share.leave());
    assert_eq!(share.directory(), r"\\host\share");
    assert!(!share.leave());
}

#[test]
fn windows_roots_are_followed_from_a_separator_a_drive_or_a_share() {
    let start = || Browser::new(Some(r"C:\Users\dev\code".to_owned()), None, Vec::new()).unwrap();
    for root in ["/", r"\"] {
        let mut browser = start();
        assert_eq!(browser.follow(root, &[]).as_deref(), Some(""));
        assert_eq!(browser.directory(), r"C:\", "{root} is the drive's root");
    }
    let mut browser = start();
    assert_eq!(browser.follow("d:/data/x", &[]).as_deref(), Some("x"));
    assert_eq!(browser.directory(), r"d:\data");

    let mut browser = start();
    assert_eq!(browser.follow(r"\\host", &[]), None);
    assert_eq!(browser.follow(r"\\host\share", &[]), None);
    assert_eq!(
        browser.directory(),
        r"C:\Users\dev\code",
        "a share half typed"
    );
    assert_eq!(
        browser.follow(r"\\host\share\docs\re", &[]).as_deref(),
        Some("re")
    );
    assert_eq!(browser.directory(), r"\\host\share\docs");
    assert_eq!(browser.follow(r"\", &[]).as_deref(), Some(""));
    assert_eq!(browser.directory(), r"\\host\share");
}

#[test]
fn a_terminal_that_changed_folder_gets_paths_from_its_new_folder() {
    let mut browser = browser(&[]);
    assert_eq!(browser.chosen("notes.md", false).typed, "notes.md");
    browser.rebase(Some("/var/log/".to_owned()));
    assert_eq!(
        browser.chosen("notes.md", false).typed,
        "/Users/dev/Projects/muxy/notes.md"
    );
    browser.rebase(Some("/Users/dev".to_owned()));
    assert_eq!(
        browser.chosen("notes.md", false).typed,
        "Projects/muxy/notes.md"
    );
    browser.rebase(None);
    assert_eq!(
        browser.chosen("docs", true).typed,
        "/Users/dev/Projects/muxy/docs/"
    );
}

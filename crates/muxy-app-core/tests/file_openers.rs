use muxy_app_core::opener::{
    FileHandler, FileResolution, FileRule, file_pattern, matches_pattern, resolve_file,
};
use muxy_app_core::settings::{OpenerSettings, Settings};

fn handlers() -> Vec<FileHandler> {
    [
        ("preview:md", vec!["*.md"]),
        ("files:view", vec!["*.md", "*.rs", "Dockerfile"]),
    ]
    .into_iter()
    .map(|(id, patterns)| FileHandler {
        id: id.into(),
        title: id.into(),
        patterns: patterns.into_iter().map(String::from).collect(),
    })
    .collect()
}

fn rule(pattern: &str, opener: &str) -> FileRule {
    FileRule {
        pattern: pattern.into(),
        opener: opener.into(),
    }
}

#[test]
fn registrations_choose_only_matching_handlers_and_never_the_project_app() {
    let handlers = handlers();
    let settings = OpenerSettings {
        file: "preview:md".into(),
        project_target: Some("com.microsoft.VSCode".into()),
        ..OpenerSettings::default()
    };
    assert_eq!(
        resolve_file("docs/readme.MD", &handlers, &settings),
        FileResolution::Choose(vec!["preview:md".into(), "files:view".into()])
    );
    assert_eq!(
        resolve_file("src/main.rs", &handlers, &settings),
        FileResolution::Handler("files:view".into())
    );
    assert_eq!(
        resolve_file("image.png", &handlers, &settings),
        FileResolution::System
    );
    assert_eq!(
        resolve_file("docs/readme.md", &[], &settings),
        FileResolution::System
    );
    assert_eq!(
        resolve_file("build/Dockerfile", &handlers, &settings),
        FileResolution::Handler("files:view".into())
    );
}

#[test]
fn ordered_custom_rules_choose_defaults_and_can_always_ask() {
    let handlers = handlers();
    let mut settings = OpenerSettings {
        file_rules: vec![
            rule("notes/*.md", "ask"),
            rule("*.md", "preview:md"),
            rule("*.custom", "files:view"),
        ],
        ..OpenerSettings::default()
    };
    assert!(matches!(
        resolve_file("notes/a.md", &handlers, &settings),
        FileResolution::Choose(_)
    ));
    assert_eq!(
        resolve_file("readme.md", &handlers, &settings),
        FileResolution::Handler("preview:md".into())
    );
    assert_eq!(
        resolve_file("file.custom", &handlers, &settings),
        FileResolution::Handler("files:view".into())
    );
    settings.file_rules.insert(0, rule("*.md", "system.finder"));
    assert_eq!(
        resolve_file("notes/a.md", &handlers, &settings),
        FileResolution::System
    );
    settings.always_ask = true;
    assert!(matches!(
        resolve_file("notes/a.md", &handlers, &settings),
        FileResolution::Choose(_)
    ));
    assert_eq!(
        resolve_file("main.rs", &handlers, &settings),
        FileResolution::Choose(vec!["files:view".into()])
    );
    assert_eq!(
        resolve_file("file.unknown", &handlers, &settings),
        FileResolution::System
    );
}

#[test]
fn missing_handlers_and_invalid_rules_cannot_capture_files() {
    let mut settings = OpenerSettings {
        file_rules: vec![rule("*.md", "removed:viewer"), rule("*", "system.editor")],
        ..OpenerSettings::default()
    };
    assert!(matches!(
        resolve_file("readme.md", &handlers(), &settings),
        FileResolution::Choose(_)
    ));
    assert_eq!(
        resolve_file("file.custom", &handlers(), &settings),
        FileResolution::System
    );
    settings.file_rules = vec![rule("*.md", "preview:md")];
    assert_eq!(
        resolve_file("readme.md", &handlers()[1..], &settings),
        FileResolution::Handler("files:view".into())
    );
    assert_eq!(
        resolve_file("readme.md", &[], &settings),
        FileResolution::System
    );
}

#[test]
fn patterns_cover_names_paths_case_and_compound_suffixes() {
    for (pattern, path) in [
        ("*.md", "docs/README.MD"),
        ("Dockerfile", "nested/Dockerfile"),
        (".env", "app/.env"),
        ("src/*.rs", "SRC/nested/lib.RS"),
        ("Read?e.md", "Readme.md"),
        ("*.tar.gz", "bundle.tar.gz"),
    ] {
        assert!(matches_pattern(pattern, path), "{pattern}: {path}");
    }
    for (pattern, path) in [
        ("", "readme.md"),
        ("*", ""),
        ("*.md", "docs.md/file.txt"),
        ("src/*.rs", "test/src/lib.rs"),
        ("?", "ab"),
    ] {
        assert!(!matches_pattern(pattern, path), "{pattern}: {path}");
    }
    let mut handlers = handlers();
    handlers[0]
        .patterns
        .extend(["*.gz".into(), "*.tar.gz".into(), "*".into()]);
    assert_eq!(file_pattern("files/ARCHIVE.TAR.GZ", &handlers), "*.tar.gz");
    assert_eq!(file_pattern("a/file.rs", &handlers), "*.rs");
    assert_eq!(file_pattern("a/.env", &handlers), ".env");
    assert_eq!(file_pattern("a/.env.local", &handlers), ".env.local");
    handlers[0].patterns.push("tsconfig.json".into());
    assert_eq!(file_pattern("a/tsconfig.json", &handlers), "tsconfig.json");
    assert_eq!(file_pattern("a/Dockerfile", &handlers), "dockerfile");
}

#[test]
fn file_rules_round_trip_and_old_settings_remain_readable() -> Result<(), Box<dyn std::error::Error>>
{
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("settings.toml");
    std::fs::write(
        &path,
        "[openers]\nfile = 'system.editor'\nproject_target = 'editor'\n[clipboard]\ncopy_on_select = true\n",
    )?;
    let mut settings = Settings::load(&path)?;
    settings.openers.file_rules = vec![rule("*.md", "preview:md"), rule("Dockerfile", "ask")];
    settings.openers.always_ask = true;
    settings.save_openers(&path)?;
    let saved = Settings::load(&path)?;
    assert_eq!(saved.openers.file_rules, settings.openers.file_rules);
    assert!(saved.openers.always_ask);
    assert!(saved.clipboard.copy_on_select);
    assert_eq!(saved.openers.project_target.as_deref(), Some("editor"));
    Ok(())
}

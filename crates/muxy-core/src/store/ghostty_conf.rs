use std::path::{Path, PathBuf};

const SPACES: [char; 2] = [' ', '\t'];

pub fn path() -> PathBuf {
    crate::prefs::app_support_dir().join("ghostty.conf")
}

fn system_config_path() -> PathBuf {
    crate::prefs::home_dir().join(".config/ghostty/config")
}

pub fn seed_if_needed() {
    seed_into(&path(), &system_config_path());
}

fn seed_into(target: &Path, source: &Path) {
    if target.exists() {
        return;
    }
    let Ok(contents) = std::fs::read_to_string(source) else {
        return;
    };
    if let Err(error) = crate::store::write_private(target, contents.as_bytes()) {
        log::warn!("failed to seed {}: {error}", target.display());
    }
}

fn update_in(text: &str, key: &str, value: &str) -> String {
    let entry = format!("{key} = {value}");
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let borrowed: Vec<&str> = lines.iter().map(String::as_str).collect();
    match line_index(&borrowed, key) {
        Some(index) => lines[index] = entry,
        None => lines.insert(0, entry),
    }
    lines.join("\n")
}

fn update_value_at(target: &Path, source: &Path, key: &str, value: &str) {
    seed_into(target, source);
    let contents = update_in(&read(target), key, value);
    if let Err(error) = crate::store::write_private(target, contents.as_bytes()) {
        log::warn!("failed to write {}: {error}", target.display());
    }
}

fn update_value(key: &str, value: &str) {
    update_value_at(&path(), &system_config_path(), key, value);
}

fn sanitize(name: &str) -> String {
    name.chars()
        .filter(|character| !matches!(character, '"' | '\n' | '\r'))
        .collect()
}

pub fn set_theme(dark: &str, light: &str) {
    update_value(
        "theme",
        &format!("dark:\"{}\",light:\"{}\"", sanitize(dark), sanitize(light)),
    );
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn split_newlines(text: &str) -> Vec<&str> {
    text.split(['\n', '\r']).collect()
}

fn line_index(lines: &[&str], key: &str) -> Option<usize> {
    lines.iter().position(|line| {
        line.trim_matches(SPACES)
            .strip_prefix(key)
            .is_some_and(|rest| rest.trim_matches(SPACES).starts_with('='))
    })
}

fn value_in(text: &str, key: &str) -> Option<String> {
    let lines = split_newlines(text);
    let index = line_index(&lines, key)?;
    let trimmed = lines[index].trim_matches(SPACES);
    let after_key = trimmed[key.len()..].trim_matches(SPACES);
    Some(after_key[1..].trim_matches(SPACES).to_owned())
}

fn read_value(key: &str) -> Option<String> {
    value_in(&read(&path()), key)
}

pub fn theme_selection() -> (Option<String>, Option<String>) {
    let Some(value) = read_value("theme") else {
        return (None, None);
    };
    parse_theme_selection(&value)
}

fn unquote(value: &str) -> &str {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn split_theme_entries(value: &str) -> Vec<String> {
    let mut entries = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for character in value.chars() {
        if character == '"' {
            quoted = !quoted;
            current.push(character);
        } else if character == ',' && !quoted {
            let entry = current.trim();
            if !entry.is_empty() {
                entries.push(entry.to_owned());
            }
            current.clear();
        } else {
            current.push(character);
        }
    }
    let entry = current.trim();
    if !entry.is_empty() {
        entries.push(entry.to_owned());
    }
    entries
}

fn parse_theme_selection(value: &str) -> (Option<String>, Option<String>) {
    let trimmed = value.trim();
    let unquoted = unquote(trimmed);
    let mut dark: Option<String> = None;
    let mut light: Option<String> = None;
    let mut fallback_parts: Vec<String> = Vec::new();

    for entry in split_theme_entries(unquoted) {
        let Some((raw_key, raw_name)) = entry.split_once(':') else {
            fallback_parts.push(entry);
            continue;
        };
        let name = unquote(raw_name.trim()).to_owned();
        match raw_key.trim().to_lowercase().as_str() {
            "dark" => dark = Some(name),
            "light" => light = Some(name),
            _ => fallback_parts.push(entry),
        }
    }

    let fallback = if fallback_parts.is_empty() {
        if dark.is_none() && light.is_none() {
            Some(unquoted.to_owned())
        } else {
            None
        }
    } else {
        Some(fallback_parts.join(","))
    };

    (dark.or_else(|| fallback.clone()), light.or(fallback))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_the_value_of_a_key() {
        let text =
            "#background-blur = 20\ntheme = dark:\"Muxy\",light:\"Muxy Light\"\nfont-size = 21\n";
        assert_eq!(
            super::value_in(text, "theme").as_deref(),
            Some("dark:\"Muxy\",light:\"Muxy Light\"")
        );
        assert_eq!(super::value_in(text, "font-size").as_deref(), Some("21"));
    }

    #[test]
    fn matches_a_key_without_spaces_and_rejects_near_misses() {
        assert_eq!(
            super::value_in("theme=Muxy", "theme").as_deref(),
            Some("Muxy")
        );
        assert_eq!(
            super::value_in("  theme  =  Muxy  ", "theme").as_deref(),
            Some("Muxy")
        );
        assert_eq!(super::value_in("theme-foo = Muxy", "theme"), None);
        assert_eq!(super::value_in("# theme = Muxy", "theme"), None);
    }

    #[test]
    fn the_reader_splits_on_every_newline_character() {
        let text = "# note\r\ntheme = \"Old\"\r\nfont-size = 21\r\n";
        assert_eq!(super::split_newlines(text).len(), 7);
        assert_eq!(super::value_in(text, "theme").as_deref(), Some("\"Old\""));
    }

    #[test]
    fn parses_the_paired_theme_form() {
        assert_eq!(
            super::parse_theme_selection("dark:\"Muxy\",light:\"Muxy Light\""),
            (Some("Muxy".to_owned()), Some("Muxy Light".to_owned()))
        );
    }

    #[test]
    fn parses_the_single_theme_form_as_both_sides() {
        assert_eq!(
            super::parse_theme_selection("\"Solarized Dark\""),
            (
                Some("Solarized Dark".to_owned()),
                Some("Solarized Dark".to_owned())
            )
        );
    }

    #[test]
    fn keeps_a_comma_inside_quotes_in_one_entry() {
        assert_eq!(
            super::parse_theme_selection("dark:\"Ayu, Mirage\",light:\"Muxy Light\""),
            (
                Some("Ayu, Mirage".to_owned()),
                Some("Muxy Light".to_owned())
            )
        );
    }

    #[test]
    fn replacing_a_line_leaves_every_other_byte_alone() {
        let text = "# comment\n\ntheme = \"Old\"\nfont-size = 21\ntheme = \"Duplicate\"\n";
        assert_eq!(
            super::update_in(text, "theme", "dark:\"A\",light:\"B\""),
            "# comment\n\ntheme = dark:\"A\",light:\"B\"\nfont-size = 21\ntheme = \"Duplicate\"\n"
        );
    }

    #[test]
    fn a_missing_key_is_inserted_at_the_first_line() {
        assert_eq!(
            super::update_in("font-size = 21\n", "theme", "\"X\""),
            "theme = \"X\"\nfont-size = 21\n"
        );
        assert_eq!(
            super::update_in("theme-foo = Muxy\n# theme = Muxy\n", "theme", "\"X\""),
            "theme = \"X\"\ntheme-foo = Muxy\n# theme = Muxy\n"
        );
    }

    #[test]
    fn the_writer_consumes_the_carriage_return_the_reader_ignores() {
        let text = "# note\r\ntheme = \"Old\"\r\nfont-size = 21\r\n";
        assert_eq!(super::value_in(text, "theme").as_deref(), Some("\"Old\""));
        assert_eq!(
            super::update_in(text, "theme", "\"New\""),
            "# note\r\ntheme = \"New\"\nfont-size = 21\r\n"
        );
    }

    #[test]
    fn a_theme_name_loses_its_quotes_and_newlines_rather_than_escaping_them() {
        assert_eq!(super::sanitize("A\"B\nC\rD"), "ABCD");
    }

    #[test]
    fn seeding_copies_the_system_config_only_when_the_target_is_absent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("ghostty.conf");
        let source = dir.path().join("config");
        std::fs::write(&source, "font-size = 30\n").expect("write");

        super::update_value_at(&target, &source, "theme", "\"X\"");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "theme = \"X\"\nfont-size = 30\n"
        );

        std::fs::write(&source, "font-size = 99\n").expect("write");
        super::update_value_at(&target, &source, "theme", "\"Y\"");
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "theme = \"Y\"\nfont-size = 30\n"
        );
    }

    #[test]
    fn seeding_does_nothing_when_the_system_config_is_absent() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("ghostty.conf");
        super::update_value_at(&target, &dir.path().join("missing"), "theme", "\"X\"");
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "theme = \"X\"\n");
    }

    #[test]
    fn an_unrecognised_entry_becomes_the_fallback() {
        assert_eq!(
            super::parse_theme_selection("dark:\"Muxy\",other:\"X\""),
            (Some("Muxy".to_owned()), Some("other:\"X\"".to_owned()))
        );
    }
}

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Tests fail immediately on fixture errors"
)]
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::json;

use super::plist::{Value, dictionary};
use super::*;

const INFO: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.example.muxy-german.localization</string>
    <key>CFBundleDevelopmentRegion</key>
    <string>de</string>
</dict>
</plist>"#;

const STRINGSDICT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>%lld changes</key>
    <dict>
        <key>NSStringLocalizedFormatKey</key>
        <string>%#@changes@</string>
        <key>changes</key>
        <dict>
            <key>NSStringFormatSpecTypeKey</key>
            <string>NSStringPluralRuleType</string>
            <key>NSStringFormatValueTypeKey</key>
            <string>lld</string>
            <key>one</key>
            <string>%lld изменение</string>
            <key>few</key>
            <string>%lld изменения</string>
            <key>many</key>
            <string>%lld изменений</string>
            <key>other</key>
            <string>%lld изменения</string>
        </dict>
    </dict>
</dict>
</plist>"#;

fn strings(entries: &[(&str, &str)]) -> BTreeMap<String, Value> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), Value::String((*value).to_owned())))
        .collect()
}

fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, contents) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
}

#[test]
fn strings_files_read_like_foundation() {
    let parsed = dictionary(
        br#"/* Settings */
"Settings" = "Einstellungen";
// Quotes and escapes
"\"Remember\" saves rule" = "\"Merken\" speichert\tRegel\n";
"Caf\U00e9" = 'octal \101';
"Emoji" = "\UD83D\UDE00";
Unquoted = value.with-dots;
"Key only";
"#,
    )
    .unwrap();
    assert_eq!(
        parsed,
        strings(&[
            ("Settings", "Einstellungen"),
            ("\"Remember\" saves rule", "\"Merken\" speichert\tRegel\n"),
            ("Café", "octal A"),
            ("Emoji", "😀"),
            ("Unquoted", "value.with-dots"),
            ("Key only", "Key only"),
        ])
    );
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(
            "\"Open\" = \"Öffnen\";"
                .encode_utf16()
                .flat_map(u16::to_le_bytes),
        )
        .collect();
    assert_eq!(dictionary(&utf16).unwrap(), strings(&[("Open", "Öffnen")]));
    assert_eq!(
        dictionary(b"{ \"A\" = \"B\"; }").unwrap(),
        strings(&[("A", "B")])
    );
    let nested = format!(
        "{}\"A\" = \"B\";{}",
        "\"k\" = {".repeat(200_000),
        "};".repeat(200_000)
    );
    assert!(dictionary(nested.as_bytes()).is_err());
    let nested = format!(
        "<plist>{}<string>x</string>{}</plist>",
        "<dict><key>k</key>".repeat(100),
        "</dict>".repeat(100)
    );
    assert!(dictionary(nested.as_bytes()).is_err());
    let entities = format!(
        "<!DOCTYPE plist [<!ENTITY big \"{}\"><!ENTITY x \"{}\">]><plist><dict><key>A</key><string>{}</string></dict></plist>",
        "A".repeat(1000),
        "&big;".repeat(255),
        "&x;".repeat(2000)
    );
    assert!(dictionary(entities.as_bytes()).is_err());
    for invalid in [
        &b"\"A\" = \"B\""[..],
        b"\"A\" = \"B",
        b"/* open",
        b"bplist00",
        b"<plist><array/></plist>",
    ] {
        assert!(dictionary(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn stringsdict_reads_as_plural_entries() {
    let parsed = dictionary(STRINGSDICT.as_bytes()).unwrap();
    let Value::Dictionary(entry) = &parsed["%lld changes"] else {
        panic!("not a plural entry");
    };
    assert_eq!(
        entry["NSStringLocalizedFormatKey"],
        Value::String("%#@changes@".into())
    );
    assert_eq!(catalog::incompatible_key(&parsed), None);
}

#[test]
fn placeholder_rules_match_mains_documentation() {
    for (key, value) in [
        ("Created branch %@", "Zweig %@ erstellt"),
        ("%@ (%@)", "%2$@ – %1$@"),
        ("%@ (%@)", "%1$@"),
        ("%ld items", "%lld Elemente"),
        ("%d items", "%hhd Elemente"),
        ("%f s", "%lf s"),
        ("Settings", "Einstellungen"),
        ("%lld%%", "%lld %%"),
    ] {
        let catalog = strings(&[(key, value)]);
        assert_eq!(catalog::incompatible_key(&catalog), None, "{key} = {value}");
    }
    for (key, value) in [
        ("Created branch %@", "Zweig %@ %@ erstellt"),
        ("%lld changes", "%@ Änderungen"),
        ("Created branch %@", "Zweig %s erstellt"),
        ("Settings", "Einstellungen %@"),
        ("%d", "%*d"),
        ("%f", "%Lf"),
        ("%@ and %@", "%1$@ %1$lld"),
    ] {
        let catalog = strings(&[(key, value)]);
        assert_eq!(
            catalog::incompatible_key(&catalog),
            Some(key),
            "{key} = {value}"
        );
    }
}

#[test]
fn plural_variants_keep_their_arguments_types() {
    let bad = STRINGSDICT.replace(
        "<string>%lld изменения</string>\n            <key>many",
        "<string>%@ изменения</string>\n            <key>many",
    );
    let catalog = dictionary(bad.as_bytes()).unwrap();
    assert_eq!(catalog::incompatible_key(&catalog), Some("%lld changes"));
    let missing = STRINGSDICT.replace("%#@changes@", "%#@absent@");
    let catalog = dictionary(missing.as_bytes()).unwrap();
    assert_eq!(catalog::incompatible_key(&catalog), Some("%lld changes"));
    let recursive = STRINGSDICT.replace(
        "<string>%lld изменений</string>",
        "<string>%#@changes@</string>",
    );
    let catalog = dictionary(recursive.as_bytes()).unwrap();
    assert_eq!(catalog::incompatible_key(&catalog), Some("%lld changes"));
}

#[test]
fn bundles_are_checked_like_main() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("German.bundle");
    write(
        &bundle,
        &[
            ("Info.plist", INFO),
            (
                "de.lproj/Localizable.strings",
                "\"Settings\" = \"Einstellungen\";",
            ),
        ],
    );
    assert_eq!(validate_bundle(&bundle, "de"), Ok(()));
    assert!(validate_bundle(&bundle, "fr").unwrap_err().contains("'fr'"));

    let executable = INFO.replace(
        "<dict>",
        "<dict><key>CFBundleExecutable</key><string>x</string>",
    );
    fs::write(bundle.join("Info.plist"), executable).unwrap();
    assert!(
        validate_bundle(&bundle, "de")
            .unwrap_err()
            .contains("executable")
    );
    fs::remove_file(bundle.join("Info.plist")).unwrap();
    assert!(
        validate_bundle(&bundle, "de")
            .unwrap_err()
            .contains("Info.plist")
    );
    fs::write(bundle.join("Info.plist"), INFO).unwrap();

    fs::write(
        bundle.join("de.lproj/Localizable.strings"),
        "\"%lld changes\" = \"%@ Änderungen\";",
    )
    .unwrap();
    assert!(
        validate_bundle(&bundle, "de")
            .unwrap_err()
            .contains("'%lld changes'")
    );
    fs::write(
        bundle.join("de.lproj/Localizable.strings"),
        "x".repeat(4 * 1024 * 1024 + 1),
    )
    .unwrap();
    assert!(
        validate_bundle(&bundle, "de")
            .unwrap_err()
            .contains("4 MiB")
    );
    #[cfg(unix)]
    {
        fs::remove_file(bundle.join("de.lproj/Localizable.strings")).unwrap();
        let outside = root.path().join("outside.strings");
        fs::write(&outside, "\"A\" = \"B\";").unwrap();
        std::os::unix::fs::symlink(&outside, bundle.join("de.lproj/Localizable.strings")).unwrap();
        assert!(validate_bundle(&bundle, "de").is_err());
    }
}

#[test]
fn enabled_extensions_provide_languages_with_cldr_plurals() {
    let profile = tempfile::tempdir().unwrap();
    let package = profile.path().join("extensions/packs");
    write(
        &package,
        &[
            (
                "package.json",
                &json!({"name":"packs","version":"1.0.0","muxy":{"localizations":[
                    {"id":"ru","language":"ru","title":"Русский","bundle":"localization/Russian.bundle"}
                ]}})
                .to_string(),
            ),
            ("localization/Russian.bundle/Info.plist", INFO),
            (
                "localization/Russian.bundle/ru.lproj/Localizable.strings",
                "\"Settings\" = \"Настройки\";",
            ),
            ("localization/Russian.bundle/ru.lproj/Localizable.stringsdict", STRINGSDICT),
        ],
    );
    let mut registry = Registry::load(profile.path());
    assert!(registry.errors.is_empty(), "{:?}", registry.errors);
    assert!(providers(&registry).is_empty());
    assert_eq!(provider(&registry, "packs:ru"), None);
    registry.set_enabled("packs", true).unwrap();

    let listed = providers(&registry);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].selection(), "packs:ru");
    assert_eq!(listed[0].title, "Русский");
    let russian = load(&provider(&registry, "packs:ru").unwrap()).unwrap();
    assert_eq!(russian.translate("Settings"), "Настройки");
    let changes = |count: i64| russian.format("%lld changes", &[count.into()]);
    assert_eq!(changes(1), "1 изменение");
    assert_eq!(changes(3), "3 изменения");
    assert_eq!(changes(5), "5 изменений");
    assert_eq!(changes(21), "21 изменение");

    assert_eq!(parse_selection("packs:ru"), Some(("packs", "ru")));
    assert_eq!(parse_selection("a:b:c"), Some(("a", "b:c")));
    for invalid in ["", "packs", ":ru", "packs:"] {
        assert_eq!(parse_selection(invalid), None, "{invalid}");
    }
    assert_eq!(provider(&registry, "packs:de"), None);
}

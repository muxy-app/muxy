#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "Tests fail immediately on fixture errors"
)]
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

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
fn oversized_format_fields_prevent_a_pack_from_loading() {
    let root = tempfile::tempdir().unwrap();
    let provider = Provider {
        extension: "pack".into(),
        id: "de".into(),
        language: "de".into(),
        title: "Deutsch".into(),
        bundle: root.path().join("German.bundle"),
    };
    for translation in [
        "%18446744073709551615lld",
        "%18446744073709551616lld",
        "%1025lld",
        "%.18446744073709551615lld",
        "%.18446744073709551616lld",
        "%.1025lld",
    ] {
        write(
            &provider.bundle,
            &[
                ("Info.plist", INFO),
                (
                    "de.lproj/Localizable.strings",
                    &format!("\"Tab %lld\" = \"Tab {translation}\";"),
                ),
            ],
        );
        assert!(load(&provider).unwrap_err().contains("Tab %lld"));
        let plural = STRINGSDICT.replace("%lld изменения", translation);
        assert_eq!(
            catalog::incompatible_key(&dictionary(plural.as_bytes()).unwrap()),
            Some("%lld changes")
        );
    }
}

#[test]
fn binary_bundles_load_strings_and_plural_rules_like_xml_bundles() {
    let root = tempfile::tempdir().unwrap();
    let bundle = root.path().join("Russian.bundle");
    fs::create_dir_all(bundle.join("ru.lproj")).unwrap();
    fs::write(
        bundle.join("Info.plist"),
        include_bytes!("fixtures/Info.plist"),
    )
    .unwrap();
    fs::write(
        bundle.join("ru.lproj/Localizable.strings"),
        include_bytes!("fixtures/Localizable.strings"),
    )
    .unwrap();
    fs::write(
        bundle.join("ru.lproj/Localizable.stringsdict"),
        include_bytes!("fixtures/Localizable.stringsdict"),
    )
    .unwrap();
    assert_eq!(validate_bundle(&bundle, "ru"), Ok(()));
    let language = load(&Provider {
        extension: "pack".into(),
        id: "ru".into(),
        language: "ru".into(),
        title: "Русский".into(),
        bundle: bundle.clone(),
    })
    .unwrap();
    assert_eq!(language.translate("Settings"), "Настройки");
    assert_eq!(language.translate("Emoji"), "😀");
    assert_eq!(
        language.translate("ASCII"),
        "A sufficiently long ASCII value"
    );
    assert_eq!(language.format("%lld changes", &[1.into()]), "1 изменение");
    assert_eq!(language.format("%lld changes", &[3.into()]), "3 изменения");
    assert_eq!(language.format("%lld changes", &[5.into()]), "5 изменений");
    assert_eq!(
        dictionary(include_bytes!("fixtures/Localizable.stringsdict")).unwrap(),
        dictionary(STRINGSDICT.as_bytes()).unwrap()
    );

    fs::write(
        bundle.join("Info.plist"),
        binary(&[b"\xd1\x01\x02", b"\x5f\x10\x12CFBundleExecutable", b"\x51x"]),
    )
    .unwrap();
    assert!(
        validate_bundle(&bundle, "ru")
            .unwrap_err()
            .contains("executable")
    );
}

fn binary(objects: &[&[u8]]) -> Vec<u8> {
    let mut bytes = b"bplist00".to_vec();
    let mut offsets = Vec::new();
    for object in objects {
        offsets.push(u64::try_from(bytes.len()).unwrap());
        bytes.extend_from_slice(object);
    }
    let table_start = u64::try_from(bytes.len()).unwrap();
    for offset in offsets {
        bytes.extend_from_slice(&offset.to_be_bytes());
    }
    bytes.extend_from_slice(&[0, 0, 0, 0, 0, 0, 8, 1]);
    bytes.extend_from_slice(&u64::try_from(objects.len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(&0_u64.to_be_bytes());
    bytes.extend_from_slice(&table_start.to_be_bytes());
    bytes
}

#[test]
fn binary_catalogs_reject_malformed_and_expanding_objects() {
    let valid = include_bytes!("fixtures/Localizable.stringsdict");
    for length in 1..valid.len() {
        assert!(
            dictionary(&valid[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }
    for objects in [
        vec![&b"\xd1\x01\xff"[..], b"\x51k"],
        vec![&b"\xd1\x01\x00"[..], b"\x51k"],
        vec![
            &b"\xd1\x01\x02"[..],
            b"\x51k",
            b"\x6f\x13\xff\xff\xff\xff\xff\xff\xff\xff",
        ],
        vec![&b"\xd1\x01\x02"[..], b"\x51k", b"\x61\xd8\x00"],
        vec![&b"\xd1\x01\x02"[..], b"\x08", b"\x51v"],
    ] {
        assert!(dictionary(&binary(&objects)).is_err());
    }
    let mut invalid_offset = binary(&[b"\xd0"]);
    invalid_offset[9..17].copy_from_slice(&u64::MAX.to_be_bytes());
    assert!(dictionary(&invalid_offset).is_err());
    let mut invalid_count = binary(&[b"\xd0"]);
    let length = invalid_count.len();
    invalid_count[length - 24..length - 16].copy_from_slice(&u64::MAX.to_be_bytes());
    assert!(dictionary(&invalid_count).is_err());

    let mut nested = Vec::new();
    for index in 0..17_u8 {
        nested.push(vec![0xd1, 18, index + 1]);
    }
    nested.push(vec![0xd0]);
    nested.push(b"\x51k".to_vec());
    let objects: Vec<&[u8]> = nested.iter().map(Vec::as_slice).collect();
    assert!(
        dictionary(&binary(&objects))
            .unwrap_err()
            .contains("deeply")
    );

    let mut references = vec![0xdf, 0x10, 32];
    references.extend_from_slice(&[1; 32]);
    references.extend_from_slice(&[2; 32]);
    let mut large_string = vec![0x5f, 0x12, 0, 0x10, 0, 0];
    large_string.extend(std::iter::repeat_n(b'x', 1024 * 1024));
    assert!(
        dictionary(&binary(&[&references, b"\x51k", &large_string]))
            .unwrap_err()
            .contains("expands")
    );
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

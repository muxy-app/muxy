use std::sync::Arc;

use super::format::tokens;
use super::{Language, Plural, PluralCategory};

fn language(strings: &[(&str, &str)], plurals: &[(&str, Plural)]) -> Language {
    Language::new(
        strings
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect(),
        plurals
            .iter()
            .map(|(key, plural)| ((*key).to_owned(), plural.clone()))
            .collect(),
        |count| match count {
            1 => PluralCategory::One,
            2..=4 => PluralCategory::Few,
            _ => PluralCategory::Other,
        },
    )
}

#[test]
fn excessive_format_fields_are_rejected_and_render_in_english() {
    for translation in [
        "Tab %18446744073709551615lld",
        "Tab %18446744073709551616lld",
        "Tab %1025lld",
        "Tab %.18446744073709551615lld",
        "Tab %.18446744073709551616lld",
        "Tab %.1025lld",
    ] {
        assert!(tokens(translation).is_none(), "accepted {translation}");
        let pack = language(&[("Tab %lld", translation)], &[]);
        assert_eq!(pack.format("Tab %lld", &[1.into()]), "Tab 1");
    }
    for invalid in ["%.1025u", "%.1025f", "%1025@"] {
        assert!(tokens(invalid).is_none(), "accepted {invalid}");
    }
    let english = language(&[], &[]);
    assert_eq!(english.format("%1024d", &[1.into()]).len(), 1024);
    assert_eq!(english.format("%.1024d", &[1.into()]).len(), 1024);
    assert_eq!(english.format("%.f", &[1.25.into()]), "1");
}

#[test]
fn translations_reorder_and_drop_arguments() {
    let german = language(
        &[
            ("%@ (%@)", "%2$@ – %1$@"),
            ("Created branch %@", "Zweig %@ erstellt"),
            ("%@ / %@", "%1$@"),
            ("Broken %@", "%"),
        ],
        &[],
    );
    assert_eq!(german.format("%@ (%@)", &["a".into(), "b".into()]), "b – a");
    assert_eq!(
        german.format("Created branch %@", &["x".into()]),
        "Zweig x erstellt"
    );
    assert_eq!(german.format("%@ / %@", &["a".into(), "b".into()]), "a");
    assert_eq!(german.format("Broken %@", &["x".into()]), "Broken x");
    assert_eq!(german.translate("Missing"), "Missing");
}

#[test]
fn the_active_language_applies_to_tr_and_falls_back_to_english() {
    assert_eq!(crate::tr!("Settings"), "Settings");
    assert_eq!(crate::tr!("%lld%%", 5), "5%");
    assert_eq!(crate::tr!("Rate 100%%"), "Rate 100%");
    for text in ["Deploy at 100% done", "date +%s", "50%", "%lld"] {
        assert_eq!(super::translate(text), text, "user text is not a format");
    }
    super::set_language(Some(Arc::new(language(
        &[
            ("Settings", "Einstellungen"),
            ("%lld changes", "%lld Änderungen"),
        ],
        &[],
    ))));
    assert_eq!(crate::tr!("Settings"), "Einstellungen");
    assert_eq!(crate::tr!("%lld changes", 2_usize), "2 Änderungen");
    assert_eq!(crate::tr!("Close"), "Close");
    assert_eq!(super::searchable("Settings"), "settings einstellungen");
    super::set_language(None);
    assert_eq!(crate::tr!("Settings"), "Settings");
    assert_eq!(crate::tr_key!("Settings"), "Settings");
}

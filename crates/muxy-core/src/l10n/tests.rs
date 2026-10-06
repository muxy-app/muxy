use std::collections::HashMap;
use std::sync::Arc;

use super::format::{Kind, Token, tokens};
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

fn plural(format: &str, variables: &[(&str, &[(PluralCategory, &str)])]) -> Plural {
    Plural {
        format: format.into(),
        variables: variables
            .iter()
            .map(|(name, forms)| {
                (
                    (*name).to_owned(),
                    forms
                        .iter()
                        .map(|(category, form)| (*category, (*form).to_owned()))
                        .collect::<HashMap<_, _>>(),
                )
            })
            .collect(),
    }
}

#[test]
fn tokens_follow_mains_placeholder_grammar() {
    let parsed = tokens("%2$@ has %lld%% and %#@files@").unwrap();
    assert!(matches!(
        parsed[0],
        Token::Argument(spec) if spec.position == Some(2) && spec.kind == Kind::Object
    ));
    assert!(matches!(
        parsed[2],
        Token::Argument(spec) if spec.position.is_none() && spec.kind == Kind::Int64
    ));
    assert_eq!(parsed[3], Token::Percent);
    assert_eq!(parsed[5], Token::Variable("files"));
    for kind_pair in [
        ("%ld", "%lld"),
        ("%d", "%hhd"),
        ("%u", "%hu"),
        ("%f", "%lf"),
    ] {
        let kind = |format| match tokens(format).unwrap()[0] {
            Token::Argument(spec) => spec.kind,
            _ => unreachable!(),
        };
        assert_eq!(kind(kind_pair.0), kind(kind_pair.1), "{kind_pair:?}");
    }
    for invalid in ["50%", "%*d", "%.*f", "%#x", "%#@@", "%0$@", "%k", "%l@"] {
        assert!(tokens(invalid).is_none(), "accepted {invalid}");
    }
}

#[test]
fn english_formats_like_printf() {
    let english = language(&[], &[]);
    assert_eq!(english.format("%lld changes", &[3.into()]), "3 changes");
    assert_eq!(
        english.format("%@ (%@)", &["main".into(), "origin".into()]),
        "main (origin)"
    );
    assert_eq!(english.format("%lld%%", &[42.into()]), "42%");
    assert_eq!(english.format("%.1f MB", &[2.25_f64.into()]), "2.2 MB");
    assert_eq!(
        english.format("%05.1f|%-4d|%+d", &[3.25_f64.into(), 7.into(), 5.into()]),
        "003.2|7   |+5"
    );
    assert_eq!(
        english.format(
            "%03lld|%x|%X|%o",
            &[(-7).into(), 255.into(), 255.into(), 8.into()]
        ),
        "-07|ff|FF|10"
    );
    assert_eq!(
        english.format(
            "%e|%g|%g",
            &[1500.0.into(), 0.0001.into(), 1_234_567.0.into()]
        ),
        "1.500000e+03|0.0001|1.23457e+06"
    );
    assert_eq!(english.format("%.2@", &["abc".into()]), "ab");
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
fn plurals_pick_a_form_by_category_with_mains_zero_rule() {
    let files = plural(
        "%#@files@ in %#@folders@",
        &[
            (
                "files",
                &[
                    (PluralCategory::Zero, "no files"),
                    (PluralCategory::One, "%lld file"),
                    (PluralCategory::Few, "%lld filesy"),
                    (PluralCategory::Other, "%lld files"),
                ],
            ),
            (
                "folders",
                &[(PluralCategory::Other, "%lld folders (%#@nested@)")],
            ),
            (
                "nested",
                &[
                    (PluralCategory::One, "one"),
                    (PluralCategory::Other, "many"),
                ],
            ),
        ],
    );
    let pack = language(&[], &[("%lld files in %lld folders", files)]);
    let format = |a: i64, b: i64| pack.format("%lld files in %lld folders", &[a.into(), b.into()]);
    assert_eq!(format(0, 1), "no files in 1 folders (many)");
    assert_eq!(format(1, 2), "1 file in 2 folders (many)");
    assert_eq!(format(3, 9), "3 filesy in 9 folders (many)");
    assert_eq!(format(-12, 1), "-12 files in 1 folders (many)");
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

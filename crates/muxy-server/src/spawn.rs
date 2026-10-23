use std::collections::BTreeMap;
use std::env;
use std::ffi::{CString, OsStr, OsString};
use std::path::{Path, PathBuf};

use muxy_protocol::ErrorCode;
use muxy_terminal::pty::{Pty, PtySize, SpawnRequest};

use crate::error::ServerError;
use crate::settings::ServerSettings;

#[cfg(target_os = "macos")]
const FALLBACK_SHELL: &str = "/bin/zsh";
#[cfg(target_os = "linux")]
const FALLBACK_SHELL: &str = "/bin/sh";
const LOGIN_FLAG: &str = "-l";
const TERMINAL_ENV: [(&str, &str); 3] = [
    ("TERM", "xterm-256color"),
    ("COLORTERM", "truecolor"),
    ("TERM_PROGRAM", "muxy"),
];

pub(crate) fn spawn_shell(
    settings: &ServerSettings,
    integration: Option<&crate::ShellIntegration>,
    directory: &Path,
    size: PtySize,
    identity: (muxy_protocol::ServerIdentity, muxy_protocol::SessionId),
    extra: &BTreeMap<String, String>,
) -> Result<Pty, ServerError> {
    if !directory.is_dir() {
        return Err(ServerError::new(
            ErrorCode::BadPath,
            format!("{} is not a directory", directory.display()),
        ));
    }
    let mut request = SpawnRequest {
        program: resolve_shell(settings),
        args: vec![OsString::from(LOGIN_FLAG)],
        cwd: directory.to_path_buf(),
        env: environment(identity, extra),
        size,
    };
    request.env.retain(|(name, _)| name != "SHELL");
    request.env.push((
        OsString::from("SHELL"),
        request.program.clone().into_os_string(),
    ));
    if let Some(integration) = integration {
        integration.configure(&mut request, settings.shell_integration);
    }
    Pty::spawn(request).map_err(ServerError::spawn_failed)
}

fn resolve_shell(settings: &ServerSettings) -> PathBuf {
    select_shell(
        settings.default_shell.clone(),
        env::var_os("SHELL"),
        cfg!(target_os = "linux"),
    )
}

fn select_shell(configured: Option<PathBuf>, inherited: Option<OsString>, linux: bool) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    configured
        .or_else(|| {
            inherited
                .filter(|shell| !shell.is_empty())
                .map(PathBuf::from)
                .filter(|shell| {
                    !linux
                        || (shell.is_absolute()
                            && shell.metadata().is_ok_and(|metadata| {
                                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
                            }))
                })
        })
        .unwrap_or_else(|| PathBuf::from(if linux { "/bin/sh" } else { FALLBACK_SHELL }))
}

/// `extra`, such as a proxy, replaces inherited variables but never Muxy's own.
fn environment(
    identity: (muxy_protocol::ServerIdentity, muxy_protocol::SessionId),
    extra: &BTreeMap<String, String>,
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<_> = env::vars_os()
        .filter(|(name, _)| name.to_str().is_none_or(|name| !extra.contains_key(name)))
        .collect();
    env.extend(
        extra
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    );
    env.retain(|(name, _)| name != "MUXY_SERVER_ID" && name != "MUXY_SESSION_ID");
    env.push(("MUXY_SERVER_ID".into(), identity.0.to_string().into()));
    env.push((
        "MUXY_SESSION_ID".into(),
        identity.1.get().to_string().into(),
    ));
    env.extend(
        TERMINAL_ENV
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    );
    apply_utf8_locale(&mut env);
    env
}

const CHARSET_LOCALE_KEYS: [&str; 3] = ["LC_ALL", "LC_CTYPE", "LANG"];
const FALLBACK_UTF8_LOCALE: &str = "C.UTF-8";

fn apply_utf8_locale(env: &mut Vec<(OsString, OsString)>) {
    ensure_utf8_locale(env, system_locale().as_deref(), locale_installed);
}

fn ensure_utf8_locale(
    env: &mut Vec<(OsString, OsString)>,
    system: Option<&str>,
    installed: impl FnMut(&str) -> bool,
) {
    if locale_is_set(env) {
        return;
    }
    let Some(value) = preferred_utf8_locale(system, installed) else {
        return;
    };
    for (name, item) in env.iter_mut() {
        if is_charset_locale_key(name) && is_blank(item) {
            item.clear();
        }
    }
    set_env(env, "LANG", &value);
}

fn preferred_utf8_locale(
    system: Option<&str>,
    mut installed: impl FnMut(&str) -> bool,
) -> Option<String> {
    system
        .and_then(language_and_region)
        .map(|locale| format!("{locale}.UTF-8"))
        .into_iter()
        .chain([String::from(FALLBACK_UTF8_LOCALE)])
        .find(|name| installed(name))
}

fn language_and_region(identifier: &str) -> Option<String> {
    let head = identifier.split(['@', '.']).next()?.trim();
    let mut subtags = head.split(['-', '_']);
    let language = subtags.next()?;
    let region = subtags.find(|subtag| !is_script(subtag))?;
    let valid = (2..=3).contains(&language.len())
        && language.bytes().all(|byte| byte.is_ascii_alphabetic())
        && (2..=3).contains(&region.len())
        && region.bytes().all(|byte| byte.is_ascii_alphanumeric());
    valid.then(|| format!("{language}_{region}"))
}

fn is_script(subtag: &str) -> bool {
    subtag.len() == 4 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn locale_is_set(env: &[(OsString, OsString)]) -> bool {
    CHARSET_LOCALE_KEYS.iter().any(|key| {
        env.iter()
            .rev()
            .find(|(name, _)| name == key)
            .is_some_and(|(_, value)| !is_blank(value))
    })
}

fn is_charset_locale_key(name: &OsStr) -> bool {
    CHARSET_LOCALE_KEYS.iter().any(|key| name == *key)
}

fn is_blank(value: &OsStr) -> bool {
    value.to_str().is_some_and(|text| text.trim().is_empty())
}

fn set_env(env: &mut Vec<(OsString, OsString)>, key: &str, value: &str) {
    env.retain(|(name, _)| name != key);
    env.push((OsString::from(key), OsString::from(value)));
}

#[allow(
    unsafe_code,
    reason = "the C library is the only authority on which locales it can load"
)]
fn locale_installed(name: &str) -> bool {
    let Ok(name) = CString::new(name) else {
        return false;
    };
    // SAFETY: `name` is a valid C string and a null base asks for a new locale.
    let locale =
        unsafe { libc::newlocale(libc::LC_CTYPE_MASK, name.as_ptr(), std::ptr::null_mut()) };
    if locale.is_null() {
        return false;
    }
    // SAFETY: `locale` came from `newlocale` and is freed once.
    unsafe { libc::freelocale(locale) };
    true
}

fn system_locale() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let identifier = objc2_foundation::NSLocale::currentLocale()
            .localeIdentifier()
            .to_string();
        (!identifier.is_empty()).then_some(identifier)
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last<'a>(env: &'a [(OsString, OsString)], name: &str) -> Option<&'a OsString> {
        env.iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }

    #[test]
    fn requested_variables_replace_inherited_ones_but_not_muxys_own() {
        let session = muxy_protocol::SessionId::new(7).expect("non-zero");
        let extra = BTreeMap::from([
            ("PATH".to_owned(), "/proxy/bin".to_owned()),
            ("http_proxy".to_owned(), "http://proxy:8080".to_owned()),
            ("LC_ALL".to_owned(), "C".to_owned()),
            ("MUXY_SESSION_ID".to_owned(), "1".to_owned()),
            ("TERM".to_owned(), "dumb".to_owned()),
        ]);
        let env = environment(
            (muxy_protocol::ServerIdentity::from_u128(1), session),
            &extra,
        );
        assert_eq!(env.iter().filter(|(key, _)| key == "PATH").count(), 1);
        assert_eq!(last(&env, "PATH"), Some(&OsString::from("/proxy/bin")));
        assert_eq!(
            last(&env, "http_proxy"),
            Some(&OsString::from("http://proxy:8080"))
        );
        assert_eq!(last(&env, "LC_ALL"), Some(&OsString::from("C")));
        assert_eq!(last(&env, "MUXY_SESSION_ID"), Some(&OsString::from("7")));
        assert_eq!(last(&env, "TERM"), Some(&OsString::from("xterm-256color")));
    }

    fn pairs(values: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        values
            .iter()
            .map(|(key, value)| (OsString::from(*key), OsString::from(*value)))
            .collect()
    }

    fn value<'a>(env: &'a [(OsString, OsString)], key: &str) -> Option<&'a str> {
        env.iter()
            .rev()
            .find(|(name, _)| name == key)
            .and_then(|(_, item)| item.to_str())
    }

    fn chinese_installed(name: &str) -> bool {
        name == "zh_CN.UTF-8" || name == FALLBACK_UTF8_LOCALE
    }

    #[test]
    fn apple_locale_becomes_installed_utf8_locale() {
        assert_eq!(
            preferred_utf8_locale(Some("zh_CN"), chinese_installed).as_deref(),
            Some("zh_CN.UTF-8")
        );
        assert_eq!(
            preferred_utf8_locale(Some(" zh_CN.GB2312 "), chinese_installed).as_deref(),
            Some("zh_CN.UTF-8")
        );
        assert_eq!(
            preferred_utf8_locale(Some("en_US@rg=cnzzzz"), |name| name == "en_US.UTF-8").as_deref(),
            Some("en_US.UTF-8")
        );
        assert_eq!(
            preferred_utf8_locale(Some("zh-Hant_CN"), chinese_installed).as_deref(),
            Some("zh_CN.UTF-8")
        );
        assert_eq!(
            preferred_utf8_locale(Some("sr-Latn_RS"), |name| name == "sr_RS.UTF-8").as_deref(),
            Some("sr_RS.UTF-8")
        );
    }

    #[test]
    fn unknown_locale_uses_installed_c_utf8() {
        assert_eq!(
            preferred_utf8_locale(Some("../tmp"), |_| true).as_deref(),
            Some(FALLBACK_UTF8_LOCALE)
        );
        assert_eq!(
            preferred_utf8_locale(Some("zz_ZZ"), |name| name == FALLBACK_UTF8_LOCALE).as_deref(),
            Some(FALLBACK_UTF8_LOCALE)
        );
        assert_eq!(
            preferred_utf8_locale(Some("zh-Hans"), chinese_installed).as_deref(),
            Some(FALLBACK_UTF8_LOCALE)
        );
        assert_eq!(
            preferred_utf8_locale(None, |name| name == FALLBACK_UTF8_LOCALE).as_deref(),
            Some(FALLBACK_UTF8_LOCALE)
        );
        assert_eq!(preferred_utf8_locale(None, |_| false), None);
    }

    #[test]
    fn explicit_locale_is_preserved() {
        let mut posix = pairs(&[("LANG", "C"), ("TERM", "xterm-256color")]);
        ensure_utf8_locale(&mut posix, Some("zh_CN"), |_| true);
        assert_eq!(value(&posix, "LANG"), Some("C"));

        let mut utf8 = pairs(&[("LC_CTYPE", "en_US.UTF-8")]);
        ensure_utf8_locale(&mut utf8, Some("zh_CN"), |_| true);
        assert_eq!(value(&utf8, "LANG"), None);
        assert_eq!(value(&utf8, "LC_CTYPE"), Some("en_US.UTF-8"));

        let mut forced = pairs(&[("LC_ALL", "C"), ("LANG", "zh_CN.UTF-8")]);
        ensure_utf8_locale(&mut forced, Some("en_US"), |_| true);
        assert_eq!(value(&forced, "LC_ALL"), Some("C"));
        assert_eq!(value(&forced, "LANG"), Some("zh_CN.UTF-8"));
    }

    #[test]
    fn blank_locale_variables_are_replaced() {
        let mut env = pairs(&[
            ("LC_ALL", ""),
            ("LC_CTYPE", "   "),
            ("LANG", ""),
            ("LC_MESSAGES", "C"),
            ("TERM", "dumb"),
            ("http_proxy", "http://proxy:8080"),
        ]);
        ensure_utf8_locale(&mut env, Some("zh_CN"), chinese_installed);
        assert_eq!(value(&env, "LANG"), Some("zh_CN.UTF-8"));
        assert_eq!(value(&env, "LC_ALL"), Some(""));
        assert_eq!(value(&env, "LC_CTYPE"), Some(""));
        assert_eq!(value(&env, "LC_MESSAGES"), Some("C"));
        assert_eq!(value(&env, "http_proxy"), Some("http://proxy:8080"));
        assert_eq!(value(&env, "TERM"), Some("dumb"));
        assert_eq!(env.iter().filter(|(key, _)| key == "LANG").count(), 1);
    }

    #[test]
    fn missing_locale_stays_unset_when_nothing_is_installed() {
        let mut env = pairs(&[("PATH", "/usr/bin"), ("LANG", "")]);
        ensure_utf8_locale(&mut env, Some("zh_CN"), |_| false);
        assert_eq!(value(&env, "LANG"), Some(""));
        assert_eq!(value(&env, "PATH"), Some("/usr/bin"));
    }

    #[test]
    fn only_loadable_locales_are_installed() {
        assert!(locale_installed("C"));
        assert!(!locale_installed("zz_ZZ.UTF-8"));
        assert!(!locale_installed("C\0UTF-8"));
    }
}

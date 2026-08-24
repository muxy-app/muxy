#[cfg(target_os = "macos")]
const SUITE: &str = "com.muxy.app";

#[cfg(target_os = "macos")]
pub fn store_bool(key: &str, value: bool) {
    use objc2::rc::Retained;
    use objc2_foundation::{NSNumber, NSString};

    let Some(defaults) = suite() else { return };
    let number: Retained<NSNumber> = NSNumber::numberWithBool(value);
    unsafe { defaults.setObject_forKey(Some(&number), &NSString::from_str(key)) };
    defaults.synchronize();
}

#[cfg(target_os = "macos")]
pub fn store_string(key: &str, value: Option<&str>) {
    use objc2_foundation::NSString;

    let Some(defaults) = suite() else { return };
    let key = NSString::from_str(key);
    match value {
        Some(value) => {
            let value = NSString::from_str(value);
            unsafe { defaults.setObject_forKey(Some(&value), &key) };
        }
        None => defaults.removeObjectForKey(&key),
    }
    defaults.synchronize();
}

#[cfg(target_os = "macos")]
pub fn store_i64(key: &str, value: i64) {
    use objc2::rc::Retained;
    use objc2_foundation::{NSNumber, NSString};

    let Some(defaults) = suite() else { return };
    let number: Retained<NSNumber> = NSNumber::numberWithLongLong(value);
    unsafe { defaults.setObject_forKey(Some(&number), &NSString::from_str(key)) };
    defaults.synchronize();
}

#[cfg(target_os = "macos")]
pub fn store_f64(key: &str, value: f64) {
    use objc2::rc::Retained;
    use objc2_foundation::{NSNumber, NSString};

    let Some(defaults) = suite() else { return };
    let number: Retained<NSNumber> = NSNumber::numberWithDouble(value);
    unsafe { defaults.setObject_forKey(Some(&number), &NSString::from_str(key)) };
    defaults.synchronize();
}

#[cfg(target_os = "macos")]
pub fn store_dictionary(key: &str, value: &std::collections::HashMap<String, String>) {
    use objc2::rc::Retained;
    use objc2_foundation::{NSDictionary, NSString};

    let Some(defaults) = suite() else { return };
    let keys: Vec<Retained<NSString>> = value.keys().map(|key| NSString::from_str(key)).collect();
    let values: Vec<Retained<NSString>> = value
        .values()
        .map(|value| NSString::from_str(value))
        .collect();
    let key_refs: Vec<&NSString> = keys.iter().map(Retained::as_ref).collect();
    let dictionary = NSDictionary::from_retained_objects(&key_refs, &values);
    unsafe { defaults.setObject_forKey(Some(&dictionary), &NSString::from_str(key)) };
    defaults.synchronize();
}

#[cfg(target_os = "macos")]
pub fn remove(key: &str) {
    use objc2_foundation::NSString;

    let Some(defaults) = suite() else { return };
    defaults.removeObjectForKey(&NSString::from_str(key));
    defaults.synchronize();
}

#[cfg(target_os = "macos")]
pub fn read_bool(key: &str) -> Option<bool> {
    Some(number(key)?.boolValue())
}

#[cfg(target_os = "macos")]
pub fn read_i64(key: &str) -> Option<i64> {
    Some(number(key)?.longLongValue())
}

#[cfg(target_os = "macos")]
pub fn read_f64(key: &str) -> Option<f64> {
    Some(number(key)?.doubleValue())
}

#[cfg(target_os = "macos")]
pub fn read_string(key: &str) -> Option<String> {
    use objc2_foundation::NSString;

    Some(object(key)?.downcast::<NSString>().ok()?.to_string())
}

#[cfg(target_os = "macos")]
fn number(key: &str) -> Option<objc2::rc::Retained<objc2_foundation::NSNumber>> {
    object(key)?.downcast::<objc2_foundation::NSNumber>().ok()
}

#[cfg(target_os = "macos")]
fn object(key: &str) -> Option<objc2::rc::Retained<objc2::runtime::AnyObject>> {
    use objc2_foundation::NSString;

    suite()?.objectForKey(&NSString::from_str(key))
}

#[cfg(target_os = "macos")]
fn prefers_standard_defaults(bundle_identifier: Option<&str>) -> bool {
    bundle_identifier == Some(SUITE)
}

#[cfg(target_os = "macos")]
fn suite() -> Option<objc2::rc::Retained<objc2_foundation::NSUserDefaults>> {
    use objc2::AnyThread;
    use objc2_foundation::{NSBundle, NSString, NSUserDefaults};

    let identifier = NSBundle::mainBundle()
        .bundleIdentifier()
        .map(|value| value.to_string());
    if prefers_standard_defaults(identifier.as_deref()) {
        return Some(NSUserDefaults::standardUserDefaults());
    }
    NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&NSString::from_str(SUITE)))
}

#[cfg(not(target_os = "macos"))]
pub fn store_bool(_key: &str, _value: bool) {}

#[cfg(not(target_os = "macos"))]
pub fn store_string(_key: &str, _value: Option<&str>) {}

#[cfg(not(target_os = "macos"))]
pub fn store_i64(_key: &str, _value: i64) {}

#[cfg(not(target_os = "macos"))]
pub fn store_f64(_key: &str, _value: f64) {}

#[cfg(not(target_os = "macos"))]
pub fn store_dictionary(_key: &str, _value: &std::collections::HashMap<String, String>) {}

#[cfg(not(target_os = "macos"))]
pub fn remove(_key: &str) {}

#[cfg(not(target_os = "macos"))]
pub fn read_bool(_key: &str) -> Option<bool> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn read_i64(_key: &str) -> Option<i64> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn read_f64(_key: &str) -> Option<f64> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn read_string(_key: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    #[cfg(target_os = "macos")]
    fn the_bundle_identifier_is_never_used_as_a_suite_name() {
        assert!(super::prefers_standard_defaults(Some(super::SUITE)));
        assert!(!super::prefers_standard_defaults(None));
        assert!(!super::prefers_standard_defaults(Some("com.example.other")));
    }

    #[cfg(target_os = "macos")]
    fn delete(key: &str) {
        std::process::Command::new("defaults")
            .args(["delete", "com.muxy.app", key])
            .output()
            .expect("defaults delete");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn writes_and_clears_user_defaults() {
        let key = "muxy.native.scratchTest";
        super::store_string(key, Some("value"));
        let output = std::process::Command::new("defaults")
            .args(["read", "com.muxy.app", key])
            .output()
            .expect("defaults read");
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "value");

        super::store_string(key, None);
        let cleared = std::process::Command::new("defaults")
            .args(["read", "com.muxy.app", key])
            .output()
            .expect("defaults read");
        assert!(!cleared.status.success());

        let dictionary_key = "muxy.native.scratchDictionaryTest";
        let mut value = std::collections::HashMap::new();
        value.insert("alpha".to_owned(), "one".to_owned());
        super::store_dictionary(dictionary_key, &value);
        let output = std::process::Command::new("defaults")
            .args(["read", "com.muxy.app", dictionary_key])
            .output()
            .expect("defaults read");
        let read = String::from_utf8_lossy(&output.stdout);
        assert!(read.contains("alpha"), "{read}");
        assert!(read.contains("one"), "{read}");

        delete(dictionary_key);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn round_trips_every_accessor() {
        let string_key = "muxy.native.scratchStringRoundTrip";
        assert_eq!(super::read_string(string_key), None);
        super::store_string(string_key, Some("hello"));
        assert_eq!(super::read_string(string_key).as_deref(), Some("hello"));

        let bool_key = "muxy.native.scratchBoolRoundTrip";
        assert_eq!(super::read_bool(bool_key), None);
        super::store_bool(bool_key, true);
        assert_eq!(super::read_bool(bool_key), Some(true));
        super::store_bool(bool_key, false);
        assert_eq!(super::read_bool(bool_key), Some(false));

        let int_key = "muxy.native.scratchIntRoundTrip";
        assert_eq!(super::read_i64(int_key), None);
        super::store_i64(int_key, 1800);
        assert_eq!(super::read_i64(int_key), Some(1800));

        let double_key = "muxy.native.scratchDoubleRoundTrip";
        assert_eq!(super::read_f64(double_key), None);
        super::store_f64(double_key, 1.25);
        assert_eq!(super::read_f64(double_key), Some(1.25));

        super::remove(string_key);
        super::remove(bool_key);
        super::remove(int_key);
        super::remove(double_key);
        assert_eq!(super::read_string(string_key), None);
        assert_eq!(super::read_bool(bool_key), None);
        assert_eq!(super::read_i64(int_key), None);
        assert_eq!(super::read_f64(double_key), None);

        for key in [string_key, bool_key, int_key, double_key] {
            std::process::Command::new("defaults")
                .args(["delete", "com.muxy.app", key])
                .output()
                .expect("defaults delete");
        }
    }
}

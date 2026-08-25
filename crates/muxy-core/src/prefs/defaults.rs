#[cfg(target_os = "macos")]
struct DefaultsStore {
    defaults: objc2::rc::Retained<objc2_foundation::NSUserDefaults>,
}

#[cfg(target_os = "macos")]
impl DefaultsStore {
    fn standard() -> Self {
        Self {
            defaults: objc2_foundation::NSUserDefaults::standardUserDefaults(),
        }
    }

    #[cfg(test)]
    fn new(defaults: objc2::rc::Retained<objc2_foundation::NSUserDefaults>) -> Self {
        Self { defaults }
    }

    fn store_bool(&self, key: &str, value: bool) {
        use objc2::rc::Retained;
        use objc2_foundation::{NSNumber, NSString};

        let number: Retained<NSNumber> = NSNumber::numberWithBool(value);
        unsafe {
            self.defaults
                .setObject_forKey(Some(&number), &NSString::from_str(key));
        }
        self.defaults.synchronize();
    }

    fn store_string(&self, key: &str, value: Option<&str>) {
        use objc2_foundation::NSString;

        let key = NSString::from_str(key);
        match value {
            Some(value) => {
                let value = NSString::from_str(value);
                unsafe { self.defaults.setObject_forKey(Some(&value), &key) };
            }
            None => self.defaults.removeObjectForKey(&key),
        }
        self.defaults.synchronize();
    }

    fn store_i64(&self, key: &str, value: i64) {
        use objc2::rc::Retained;
        use objc2_foundation::{NSNumber, NSString};

        let number: Retained<NSNumber> = NSNumber::numberWithLongLong(value);
        unsafe {
            self.defaults
                .setObject_forKey(Some(&number), &NSString::from_str(key));
        }
        self.defaults.synchronize();
    }

    fn store_f64(&self, key: &str, value: f64) {
        use objc2::rc::Retained;
        use objc2_foundation::{NSNumber, NSString};

        let number: Retained<NSNumber> = NSNumber::numberWithDouble(value);
        unsafe {
            self.defaults
                .setObject_forKey(Some(&number), &NSString::from_str(key));
        }
        self.defaults.synchronize();
    }

    fn store_dictionary(&self, key: &str, value: &std::collections::HashMap<String, String>) {
        use objc2::rc::Retained;
        use objc2_foundation::{NSDictionary, NSString};

        let keys: Vec<Retained<NSString>> =
            value.keys().map(|key| NSString::from_str(key)).collect();
        let values: Vec<Retained<NSString>> = value
            .values()
            .map(|value| NSString::from_str(value))
            .collect();
        let key_refs: Vec<&NSString> = keys.iter().map(Retained::as_ref).collect();
        let dictionary = NSDictionary::from_retained_objects(&key_refs, &values);
        unsafe {
            self.defaults
                .setObject_forKey(Some(&dictionary), &NSString::from_str(key));
        }
        self.defaults.synchronize();
    }

    fn remove(&self, key: &str) {
        use objc2_foundation::NSString;

        self.defaults.removeObjectForKey(&NSString::from_str(key));
        self.defaults.synchronize();
    }

    fn read_bool(&self, key: &str) -> Option<bool> {
        Some(self.number(key)?.boolValue())
    }

    fn read_i64(&self, key: &str) -> Option<i64> {
        Some(self.number(key)?.longLongValue())
    }

    fn read_f64(&self, key: &str) -> Option<f64> {
        Some(self.number(key)?.doubleValue())
    }

    fn read_string(&self, key: &str) -> Option<String> {
        use objc2_foundation::NSString;

        Some(self.object(key)?.downcast::<NSString>().ok()?.to_string())
    }

    fn number(&self, key: &str) -> Option<objc2::rc::Retained<objc2_foundation::NSNumber>> {
        self.object(key)?
            .downcast::<objc2_foundation::NSNumber>()
            .ok()
    }

    fn object(&self, key: &str) -> Option<objc2::rc::Retained<objc2::runtime::AnyObject>> {
        use objc2_foundation::NSString;

        self.defaults.objectForKey(&NSString::from_str(key))
    }
}

#[cfg(target_os = "macos")]
pub(super) fn domain_name() -> Option<String> {
    objc2_foundation::NSBundle::mainBundle()
        .bundleIdentifier()
        .map(|value| value.to_string())
}

#[cfg(target_os = "macos")]
pub fn store_bool(key: &str, value: bool) {
    DefaultsStore::standard().store_bool(key, value);
}

#[cfg(target_os = "macos")]
pub fn store_string(key: &str, value: Option<&str>) {
    DefaultsStore::standard().store_string(key, value);
}

#[cfg(target_os = "macos")]
pub fn store_i64(key: &str, value: i64) {
    DefaultsStore::standard().store_i64(key, value);
}

#[cfg(target_os = "macos")]
pub fn store_f64(key: &str, value: f64) {
    DefaultsStore::standard().store_f64(key, value);
}

#[cfg(target_os = "macos")]
pub fn store_dictionary(key: &str, value: &std::collections::HashMap<String, String>) {
    DefaultsStore::standard().store_dictionary(key, value);
}

#[cfg(target_os = "macos")]
pub fn remove(key: &str) {
    DefaultsStore::standard().remove(key);
}

#[cfg(target_os = "macos")]
pub fn read_bool(key: &str) -> Option<bool> {
    DefaultsStore::standard().read_bool(key)
}

#[cfg(target_os = "macos")]
pub fn read_i64(key: &str) -> Option<i64> {
    DefaultsStore::standard().read_i64(key)
}

#[cfg(target_os = "macos")]
pub fn read_f64(key: &str) -> Option<f64> {
    DefaultsStore::standard().read_f64(key)
}

#[cfg(target_os = "macos")]
pub fn read_string(key: &str) -> Option<String> {
    DefaultsStore::standard().read_string(key)
}

#[cfg(not(target_os = "macos"))]
pub(super) fn domain_name() -> Option<String> {
    None
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
    #[cfg(target_os = "macos")]
    struct TestDefaults {
        store: super::DefaultsStore,
        suite_name: objc2::rc::Retained<objc2_foundation::NSString>,
    }

    #[cfg(target_os = "macos")]
    impl TestDefaults {
        fn new() -> Self {
            use objc2::AnyThread;
            use objc2_foundation::{NSString, NSUserDefaults};
            use std::time::{SystemTime, UNIX_EPOCH};

            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let suite_name = NSString::from_str(&format!(
                "muxy.tests.rust.{}.{}",
                std::process::id(),
                unique
            ));
            let defaults = NSUserDefaults::initWithSuiteName(
                NSUserDefaults::alloc(),
                Some(suite_name.as_ref()),
            )
            .unwrap();
            defaults.removePersistentDomainForName(&suite_name);
            Self {
                store: super::DefaultsStore::new(defaults),
                suite_name,
            }
        }
    }

    #[cfg(target_os = "macos")]
    impl Drop for TestDefaults {
        fn drop(&mut self) {
            self.store
                .defaults
                .removePersistentDomainForName(&self.suite_name);
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn round_trips_every_accessor_in_an_isolated_suite() {
        use objc2_foundation::{NSDictionary, NSString};

        let defaults = TestDefaults::new();
        let store = &defaults.store;

        store.store_string("string", Some("hello"));
        assert_eq!(store.read_string("string").as_deref(), Some("hello"));

        store.store_bool("bool", true);
        assert_eq!(store.read_bool("bool"), Some(true));
        store.store_bool("bool", false);
        assert_eq!(store.read_bool("bool"), Some(false));

        store.store_i64("integer", 1800);
        assert_eq!(store.read_i64("integer"), Some(1800));

        store.store_f64("double", 1.25);
        assert_eq!(store.read_f64("double"), Some(1.25));

        let mut value = std::collections::HashMap::new();
        value.insert("alpha".to_owned(), "one".to_owned());
        store.store_dictionary("dictionary", &value);
        let dictionary = store
            .object("dictionary")
            .unwrap()
            .downcast::<NSDictionary>()
            .unwrap();
        let dictionary = unsafe { dictionary.cast_unchecked::<NSString, NSString>() };
        assert_eq!(
            dictionary
                .objectForKey(&NSString::from_str("alpha"))
                .unwrap()
                .to_string(),
            "one"
        );

        for key in ["string", "bool", "integer", "double", "dictionary"] {
            store.remove(key);
            assert!(store.object(key).is_none());
        }
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn string_none_removes_the_value() {
        let defaults = TestDefaults::new();
        defaults.store.store_string("string", Some("value"));
        defaults.store.store_string("string", None);
        assert_eq!(defaults.store.read_string("string"), None);
    }
}

use std::cmp::Ordering;

struct Version<'a> {
    release: [u64; 3],
    pre: Option<&'a str>,
}

impl<'a> Version<'a> {
    fn parse(value: &'a str) -> Option<Self> {
        let value = if let Some((value, build)) = value.split_once('+') {
            identifiers(build, false).then_some(value)?
        } else {
            value
        };
        let (release, pre) = match value.split_once('-') {
            Some((release, pre)) => identifiers(pre, true).then_some((release, Some(pre)))?,
            None => (value, None),
        };
        let mut parts = release.split('.');
        let mut release = [0; 3];
        for part in &mut release {
            let value = parts.next()?;
            if !numeric(value) || (value.len() > 1 && value.starts_with('0')) {
                return None;
            }
            *part = value.parse().ok()?;
        }
        parts.next().is_none().then_some(Self { release, pre })
    }

    fn compare(&self, other: &Self) -> Ordering {
        self.release
            .cmp(&other.release)
            .then_with(|| match (self.pre, other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(left), Some(right)) => {
                    let mut left = left.split('.');
                    let mut right = right.split('.');
                    loop {
                        let order = match (left.next(), right.next()) {
                            (None, None) => return Ordering::Equal,
                            (None, Some(_)) => return Ordering::Less,
                            (Some(_), None) => return Ordering::Greater,
                            (Some(left), Some(right)) => match (numeric(left), numeric(right)) {
                                (true, true) => {
                                    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
                                }
                                (true, false) => Ordering::Less,
                                (false, true) => Ordering::Greater,
                                (false, false) => left.cmp(right),
                            },
                        };
                        if order != Ordering::Equal {
                            return order;
                        }
                    }
                }
            })
    }
}

fn numeric(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn identifiers(value: &str, pre: bool) -> bool {
    value.split('.').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && !(pre && numeric(part) && part.len() > 1 && part.starts_with('0'))
    })
}

pub(crate) fn is_update(installed: &str, available: &str) -> bool {
    match (Version::parse(installed), Version::parse(available)) {
        (Some(installed), Some(available)) => available.compare(&installed).is_gt(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::is_update;

    #[test]
    fn updates_use_numeric_versions_and_prerelease_precedence() {
        for (installed, available) in [
            ("1.0.3", "1.0.4"),
            ("1.9.0", "1.10.0"),
            ("1.0.0-beta.9", "1.0.0-beta.10"),
            ("1.0.0-beta", "1.0.0-beta.1"),
            ("1.0.0-1", "1.0.0-alpha"),
            ("1.0.0-rc.1", "1.0.0"),
        ] {
            assert!(is_update(installed, available));
            assert!(!is_update(available, installed));
            assert!(!is_update(installed, installed));
        }
        assert!(!is_update("1.0.0+one", "1.0.0+two"));
        for invalid in [
            "",
            "latest",
            "1.0",
            "01.0.0",
            "1.0.0-",
            "1.0.0-01",
            "1.0.0+",
            "1.0.0+two+three",
        ] {
            assert!(!is_update(invalid, "2.0.0"));
            assert!(!is_update("1.0.0", invalid));
        }
    }
}

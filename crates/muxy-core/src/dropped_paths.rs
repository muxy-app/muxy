use std::path::Path;

pub fn parse(file_values: &[String], plain_text: Option<&str>) -> Vec<String> {
    parse_with(file_values, plain_text, |path| Path::new(path).exists())
}

pub fn parse_with(
    file_values: &[String],
    plain_text: Option<&str>,
    mut file_exists: impl FnMut(&str) -> bool,
) -> Vec<String> {
    let listed: Vec<String> = file_values
        .iter()
        .filter_map(|value| listed_path(value))
        .collect();
    if !listed.is_empty() {
        return listed;
    }

    let Some(plain_text) = plain_text else {
        return Vec::new();
    };
    let candidates: Vec<&str> = plain_text
        .split([
            '\n', '\r', '\u{000B}', '\u{000C}', '\u{0085}', '\u{2028}', '\u{2029}',
        ])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    let mut paths = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if candidate.starts_with("file://") {
            let Some(path) = decode_file_url(candidate) else {
                return Vec::new();
            };
            paths.push(path);
        } else if Path::new(candidate).is_absolute() && file_exists(candidate) {
            paths.push(candidate.to_owned());
        } else {
            return Vec::new();
        }
    }
    paths
}

fn listed_path(value: &str) -> Option<String> {
    if value.starts_with("file://") {
        return decode_file_url(value);
    }
    if value.contains("://") {
        return None;
    }
    Path::new(value).is_absolute().then(|| value.to_owned())
}

fn decode_file_url(value: &str) -> Option<String> {
    let remainder = value.strip_prefix("file://")?;
    let path = if remainder.starts_with('/') {
        remainder
    } else {
        let slash = remainder.find('/')?;
        &remainder[slash..]
    };
    let path_end = path.find(['?', '#']).unwrap_or(path.len());
    percent_decode(&path[..path_end])
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex(bytes.get(index + 1).copied()?)?;
            let low = hex(bytes.get(index + 2).copied()?)?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    fn values(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn dropped_paths_file_values_are_returned_as_filesystem_paths() {
        assert_eq!(
            super::parse_with(&values(&["file:///tmp/a.txt", "/tmp/b.txt"]), None, |_| {
                false
            }),
            values(&["/tmp/a.txt", "/tmp/b.txt"])
        );
    }

    #[test]
    fn dropped_paths_non_file_urls_are_filtered() {
        assert_eq!(
            super::parse_with(
                &values(&["https://example.com", "file:///tmp/a.txt"]),
                None,
                |_| false
            ),
            values(&["/tmp/a.txt"])
        );
    }

    #[test]
    fn dropped_paths_empty_inputs_are_empty() {
        assert!(super::parse_with(&[], None, |_| true).is_empty());
        assert!(super::parse_with(&[], Some(""), |_| true).is_empty());
        assert!(super::parse_with(&[], Some(" \n\t"), |_| true).is_empty());
    }

    #[test]
    fn dropped_paths_file_url_strings_are_decoded() {
        assert_eq!(
            super::parse_with(&[], Some("file:///tmp/a%20b.txt"), |_| false),
            values(&["/tmp/a b.txt"])
        );
        assert_eq!(
            super::parse_with(&[], Some("file://localhost/tmp/caf%C3%A9.txt"), |_| false),
            values(&["/tmp/café.txt"])
        );
    }

    #[test]
    fn dropped_paths_existing_absolute_paths_are_accepted() {
        assert_eq!(
            super::parse_with(&[], Some("/tmp/a.txt"), |path| path == "/tmp/a.txt"),
            values(&["/tmp/a.txt"])
        );
    }

    #[test]
    fn dropped_paths_missing_absolute_path_rejects_the_batch() {
        assert!(super::parse_with(&[], Some("/tmp/missing.txt"), |_| false).is_empty());
    }

    #[test]
    fn dropped_paths_mixed_valid_and_invalid_lines_reject_the_batch() {
        assert!(
            super::parse_with(&[], Some("/tmp/a.txt\nrandom log line\n/tmp/b.txt"), |_| {
                true
            })
            .is_empty()
        );
    }

    #[test]
    fn dropped_paths_multiple_valid_lines_preserve_order() {
        assert_eq!(
            super::parse_with(&[], Some("/tmp/a.txt\nfile:///tmp/b.txt"), |_| true),
            values(&["/tmp/a.txt", "/tmp/b.txt"])
        );
    }

    #[test]
    fn dropped_paths_file_values_take_precedence_over_plain_text() {
        assert_eq!(
            super::parse_with(&values(&["file:///tmp/a.txt"]), Some("/tmp/b.txt"), |_| {
                true
            }),
            values(&["/tmp/a.txt"])
        );
    }

    #[test]
    fn dropped_paths_non_file_only_list_falls_back_to_plain_text() {
        assert_eq!(
            super::parse_with(
                &values(&["https://example.com"]),
                Some("/tmp/b.txt"),
                |_| true
            ),
            values(&["/tmp/b.txt"])
        );
    }

    #[test]
    fn dropped_paths_non_path_text_is_rejected() {
        assert!(super::parse_with(&[], Some("hello world"), |_| true).is_empty());
    }

    #[test]
    fn dropped_paths_whitespace_is_trimmed() {
        assert_eq!(
            super::parse_with(&[], Some("   /tmp/a.txt  \n  /tmp/b.txt  "), |_| true),
            values(&["/tmp/a.txt", "/tmp/b.txt"])
        );
    }

    #[test]
    fn dropped_paths_malformed_file_url_rejects_plain_text_batch() {
        for value in ["file://", "file:///tmp/%GG", "file:///tmp/%FF"] {
            assert!(super::parse_with(&[], Some(value), |_| true).is_empty());
        }
    }
}

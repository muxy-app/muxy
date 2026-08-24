#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Yaml {
    Map(Vec<(String, Yaml)>),
    Seq(Vec<Yaml>),
    Str(String),
    Null,
}

impl Yaml {
    pub fn get(&self, key: &str) -> Option<&Yaml> {
        match self {
            Self::Map(entries) => entries
                .iter()
                .rev()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_seq(&self) -> Option<&[Yaml]> {
        match self {
            Self::Seq(values) => Some(values),
            _ => None,
        }
    }

    pub fn is_map(&self) -> bool {
        matches!(self, Self::Map(_))
    }
}

struct Line {
    indent: usize,
    content: String,
    number: usize,
}

pub fn parse(source: &str) -> Option<Yaml> {
    let lines = scan(source)?;
    if lines.is_empty() {
        return Some(Yaml::Null);
    }
    let mut cursor = 0;
    let value = parse_block(&lines, &mut cursor, lines[0].indent)?;
    if cursor != lines.len() {
        log::warn!(
            "layout yaml: unexpected content at line {}",
            lines[cursor].number
        );
        return None;
    }
    Some(value)
}

fn scan(source: &str) -> Option<Vec<Line>> {
    let mut lines = Vec::new();
    for (index, raw) in source.lines().enumerate() {
        let number = index + 1;
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        if raw[..indent].contains('\t') {
            log::warn!("layout yaml: tab in indentation at line {number}");
            return None;
        }
        let body = strip_comment(&raw[indent..]);
        let body = body.trim_end();
        if body.is_empty() {
            continue;
        }
        if body == "---" {
            if lines.is_empty() {
                continue;
            }
            log::warn!("layout yaml: multiple documents are not supported (line {number})");
            return None;
        }
        if body.starts_with('&') || body.starts_with('*') {
            log::warn!("layout yaml: anchors and aliases are not supported (line {number})");
            return None;
        }
        lines.push(Line {
            indent,
            content: body.to_owned(),
            number,
        });
    }
    Some(lines)
}

fn strip_comment(value: &str) -> &str {
    let bytes = value.as_bytes();
    let mut quote: Option<u8> = None;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(open) => {
                if byte == b'\\' && open == b'"' {
                    index += 2;
                    continue;
                }
                if byte == open {
                    quote = None;
                }
            }
            None => {
                if byte == b'\'' || byte == b'"' {
                    quote = Some(byte);
                } else if byte == b'#' && (index == 0 || bytes[index - 1] == b' ') {
                    return &value[..index];
                }
            }
        }
        index += 1;
    }
    value
}

fn parse_block(lines: &[Line], cursor: &mut usize, indent: usize) -> Option<Yaml> {
    if *cursor >= lines.len() {
        return Some(Yaml::Null);
    }
    if lines[*cursor].content.starts_with("- ") || lines[*cursor].content == "-" {
        return parse_sequence(lines, cursor, indent);
    }
    parse_mapping(lines, cursor, indent)
}

fn parse_sequence(lines: &[Line], cursor: &mut usize, indent: usize) -> Option<Yaml> {
    let mut items = Vec::new();
    while *cursor < lines.len() && lines[*cursor].indent == indent {
        let line = &lines[*cursor];
        let Some(rest) = strip_dash(&line.content) else {
            break;
        };
        let number = line.number;
        *cursor += 1;
        if rest.is_empty() {
            let child_indent = child_indent(lines, *cursor, indent);
            match child_indent {
                Some(child) => items.push(parse_block(lines, cursor, child)?),
                None => items.push(Yaml::Null),
            }
            continue;
        }
        if let Some((key, value)) = split_key(rest) {
            let entry_indent = indent + (line.content.len() - rest.len());
            let mut entries = Vec::new();
            entries.push(parse_entry(
                lines,
                cursor,
                key,
                value,
                entry_indent,
                number,
            )?);
            while *cursor < lines.len() && lines[*cursor].indent == entry_indent {
                let line = &lines[*cursor];
                if strip_dash(&line.content).is_some() {
                    break;
                }
                let Some((key, value)) = split_key(&line.content) else {
                    log::warn!(
                        "layout yaml: expected a mapping entry at line {}",
                        line.number
                    );
                    return None;
                };
                let number = line.number;
                *cursor += 1;
                entries.push(parse_entry(
                    lines,
                    cursor,
                    key,
                    value,
                    entry_indent,
                    number,
                )?);
            }
            items.push(Yaml::Map(entries));
            continue;
        }
        items.push(scalar(rest, number)?);
    }
    Some(Yaml::Seq(items))
}

fn strip_dash(content: &str) -> Option<&str> {
    if content == "-" {
        return Some("");
    }
    content.strip_prefix("- ").map(str::trim_start)
}

fn parse_mapping(lines: &[Line], cursor: &mut usize, indent: usize) -> Option<Yaml> {
    let mut entries = Vec::new();
    while *cursor < lines.len() && lines[*cursor].indent == indent {
        let line = &lines[*cursor];
        if strip_dash(&line.content).is_some() {
            break;
        }
        let Some((key, value)) = split_key(&line.content) else {
            log::warn!(
                "layout yaml: expected a mapping entry at line {}",
                line.number
            );
            return None;
        };
        let number = line.number;
        *cursor += 1;
        entries.push(parse_entry(lines, cursor, key, value, indent, number)?);
    }
    if entries.is_empty() {
        log::warn!("layout yaml: expected a mapping");
        return None;
    }
    Some(Yaml::Map(entries))
}

fn parse_entry(
    lines: &[Line],
    cursor: &mut usize,
    key: String,
    value: &str,
    indent: usize,
    number: usize,
) -> Option<(String, Yaml)> {
    if !value.is_empty() {
        return Some((key, scalar(value, number)?));
    }
    let Some(child) = child_indent(lines, *cursor, indent) else {
        return Some((key, Yaml::Null));
    };
    Some((key, parse_block(lines, cursor, child)?))
}

fn child_indent(lines: &[Line], cursor: usize, indent: usize) -> Option<usize> {
    let next = lines.get(cursor)?;
    if next.indent > indent {
        return Some(next.indent);
    }
    if next.indent == indent && strip_dash(&next.content).is_some() {
        return Some(indent);
    }
    None
}

fn split_key(content: &str) -> Option<(String, &str)> {
    let bytes = content.as_bytes();
    let mut quote: Option<u8> = None;
    for index in 0..bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(open) => {
                if byte == b'\\' && open == b'"' {
                    continue;
                }
                if byte == open {
                    quote = None;
                }
            }
            None => {
                if byte == b'\'' || byte == b'"' {
                    quote = Some(byte);
                } else if byte == b':' && (index + 1 == bytes.len() || bytes[index + 1] == b' ') {
                    let key = unquote(content[..index].trim_end())?;
                    return Some((key, content[index + 1..].trim_start()));
                }
            }
        }
    }
    None
}

fn scalar(value: &str, number: usize) -> Option<Yaml> {
    let value = value.trim();
    if value.is_empty() {
        return Some(Yaml::Null);
    }
    if value.starts_with('[') {
        return flow_sequence(value, number);
    }
    if value.starts_with('|') || value.starts_with('>') {
        log::warn!("layout yaml: block scalars are not supported (line {number})");
        return None;
    }
    if value.starts_with('!') || value.starts_with('&') || value.starts_with('*') {
        log::warn!("layout yaml: tags, anchors and aliases are not supported (line {number})");
        return None;
    }
    if value.starts_with('{') {
        log::warn!("layout yaml: flow mappings are not supported (line {number})");
        return None;
    }
    Some(Yaml::Str(unquote(value)?))
}

fn flow_sequence(value: &str, number: usize) -> Option<Yaml> {
    let Some(inner) = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        log::warn!("layout yaml: unterminated flow sequence at line {number}");
        return None;
    };
    if inner.trim().is_empty() {
        return Some(Yaml::Seq(Vec::new()));
    }
    let mut items = Vec::new();
    for part in split_flow(inner) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        items.push(Yaml::Str(unquote(part)?));
    }
    Some(Yaml::Seq(items))
}

fn split_flow(value: &str) -> Vec<&str> {
    let bytes = value.as_bytes();
    let mut parts = Vec::new();
    let mut quote: Option<u8> = None;
    let mut start = 0;
    for index in 0..bytes.len() {
        let byte = bytes[index];
        match quote {
            Some(open) => {
                if byte == open {
                    quote = None;
                }
            }
            None => {
                if byte == b'\'' || byte == b'"' {
                    quote = Some(byte);
                } else if byte == b',' {
                    parts.push(&value[start..index]);
                    start = index + 1;
                }
            }
        }
    }
    parts.push(&value[start..]);
    parts
}

fn unquote(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        return Some(value[1..value.len() - 1].replace("''", "'"));
    }
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        let mut out = String::new();
        let mut chars = value[1..value.len() - 1].chars();
        while let Some(character) = chars.next() {
            if character != '\\' {
                out.push(character);
                continue;
            }
            let escape = chars.next()?;
            let escaped = escaped_character(&mut chars, escape)?;
            out.push(escaped);
        }
        return Some(out);
    }
    Some(value.to_owned())
}

fn escaped_character(chars: &mut std::str::Chars<'_>, escape: char) -> Option<char> {
    match escape {
        '0' => Some('\0'),
        'a' => Some('\u{7}'),
        'b' => Some('\u{8}'),
        't' => Some('\t'),
        'n' => Some('\n'),
        'v' => Some('\u{b}'),
        'f' => Some('\u{c}'),
        'r' => Some('\r'),
        'e' => Some('\u{1b}'),
        ' ' => Some(' '),
        '"' => Some('"'),
        '/' => Some('/'),
        '\\' => Some('\\'),
        'N' => Some('\u{85}'),
        '_' => Some('\u{a0}'),
        'L' => Some('\u{2028}'),
        'P' => Some('\u{2029}'),
        'x' => escaped_code_point(chars, 2),
        'u' => escaped_code_point(chars, 4),
        'U' => escaped_code_point(chars, 8),
        _ => None,
    }
}

fn escaped_code_point(chars: &mut std::str::Chars<'_>, digits: usize) -> Option<char> {
    let mut value = 0u32;
    for _ in 0..digits {
        value = value.checked_mul(16)?;
        value = value.checked_add(chars.next()?.to_digit(16)?)?;
    }
    char::from_u32(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(entries: Vec<(&str, Yaml)>) -> Yaml {
        Yaml::Map(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    }

    fn text(value: &str) -> Yaml {
        Yaml::Str(value.to_owned())
    }

    #[test]
    fn parses_the_single_pane_schema_document() {
        let parsed = parse("tab:\n  name: editor\n  command: nvim\n").expect("parses");
        assert_eq!(
            parsed,
            map(vec![(
                "tab",
                map(vec![("name", text("editor")), ("command", text("nvim"))])
            )])
        );
    }

    #[test]
    fn parses_the_two_pane_horizontal_document() {
        let source = "layout: horizontal\npanes:\n  - tab:\n      name: editor\n      command: nvim\n  - tab:\n      name: shell\n";
        let parsed = parse(source).expect("parses");
        assert_eq!(
            parsed,
            map(vec![
                ("layout", text("horizontal")),
                (
                    "panes",
                    Yaml::Seq(vec![
                        map(vec![(
                            "tab",
                            map(vec![("name", text("editor")), ("command", text("nvim"))])
                        )]),
                        map(vec![("tab", map(vec![("name", text("shell"))]))]),
                    ])
                )
            ])
        );
    }

    #[test]
    fn parses_nested_splits_three_deep() {
        let source = "layout: horizontal\npanes:\n  - tab:\n      name: editor\n      command: nvim\n  - layout: vertical\n    panes:\n      - tab:\n          name: logs\n          command: tail -f /tmp/app.log\n      - tab:\n          name: btop\n          command: btop\n";
        let parsed = parse(source).expect("parses");
        let panes = parsed.get("panes").and_then(Yaml::as_seq).expect("panes");
        assert_eq!(panes.len(), 2);
        assert_eq!(
            panes[1].get("layout").and_then(Yaml::as_str),
            Some("vertical")
        );
        let nested = panes[1]
            .get("panes")
            .and_then(Yaml::as_seq)
            .expect("nested");
        assert_eq!(nested.len(), 2);
        assert_eq!(
            nested[0]
                .get("tab")
                .and_then(|tab| tab.get("command"))
                .and_then(Yaml::as_str),
            Some("tail -f /tmp/app.log")
        );
    }

    #[test]
    fn parses_a_bare_command_tab_and_a_list_command() {
        assert_eq!(parse("tab: htop\n"), Some(map(vec![("tab", text("htop"))])));
        let parsed = parse("tab:\n  name: setup\n  command:\n    - cd src\n    - npm install\n")
            .expect("parses");
        assert_eq!(
            parsed
                .get("tab")
                .and_then(|tab| tab.get("command"))
                .and_then(Yaml::as_seq),
            Some(&[text("cd src"), text("npm install")][..])
        );
    }

    #[test]
    fn parses_the_legacy_tabs_document() {
        let parsed =
            parse("tabs:\n  - name: editor\n    command: nvim\n  - name: shell\n").expect("parses");
        assert_eq!(
            parsed,
            map(vec![(
                "tabs",
                Yaml::Seq(vec![
                    map(vec![("name", text("editor")), ("command", text("nvim"))]),
                    map(vec![("name", text("shell"))]),
                ])
            )])
        );
    }

    #[test]
    fn parses_every_examples_document() {
        for source in [
            "tab:\n  name: shell\n",
            "layout: horizontal\npanes:\n  - tab:\n      name: editor\n      command: nvim .\n  - tab:\n      name: shell\n",
            "layout: vertical\npanes:\n  - tab:\n      name: top\n  - tab:\n      name: bottom\n",
            "layout: horizontal\npanes:\n  - tab:\n      name: left\n  - tab:\n      name: mid\n  - tab:\n      name: right\n",
            "layout: horizontal\npanes:\n  - layout: vertical\n    panes:\n      - tab:\n          name: tl\n      - tab:\n          name: bl\n  - layout: vertical\n    panes:\n      - tab:\n          name: tr\n      - tab:\n          name: br\n",
            "layout: horizontal\npanes:\n  - tab:\n      name: editor\n      command: nvim .\n  - layout: vertical\n    panes:\n      - tab:\n          name: top\n          command: top\n      - tab:\n          name: shell\n",
        ] {
            assert!(parse(source).is_some(), "{source}");
        }
    }

    #[test]
    fn handles_comments_quoting_and_flow_sequences() {
        let parsed = parse(
            "--- \n# leading comment\ntab:\n  name: \"a: b # c\"  # trailing\n  command: ['one', \"two\"]\n",
        )
        .expect("parses");
        let tab = parsed.get("tab").expect("tab");
        assert_eq!(tab.get("name").and_then(Yaml::as_str), Some("a: b # c"));
        assert_eq!(
            tab.get("command").and_then(Yaml::as_seq),
            Some(&[text("one"), text("two")][..])
        );
    }

    #[test]
    fn decodes_standard_double_quoted_escapes() {
        assert_eq!(
            unquote(r#""\0\a\b\t\n\v\f\r\e\ \"\/\\\N\_\L\P\x41\u0042\U0001F600""#),
            Some(
                "\0\u{7}\u{8}\t\n\u{b}\u{c}\r\u{1b} \"/\\\u{85}\u{a0}\u{2028}\u{2029}AB😀"
                    .to_owned(),
            )
        );
    }

    #[test]
    fn rejects_unknown_or_invalid_double_quoted_escapes() {
        assert_eq!(unquote(r#""\q""#), None);
        assert_eq!(unquote(r#""\xzz""#), None);
        assert_eq!(unquote(r#""\uD800""#), None);
    }

    #[test]
    fn duplicate_keys_resolve_last_wins() {
        let parsed = parse("layout: horizontal\nlayout: vertical\n").expect("parses");
        assert_eq!(
            parsed.get("layout").and_then(Yaml::as_str),
            Some("vertical")
        );
    }

    #[test]
    fn refuses_anchors_aliases_tags_block_scalars_and_multiple_documents() {
        assert_eq!(parse("base: &anchor\n  name: a\n"), None);
        assert_eq!(parse("tab: *anchor\n"), None);
        assert_eq!(parse("name: !!str hello\n"), None);
        assert_eq!(parse("command: |\n  one\n  two\n"), None);
        assert_eq!(parse("tab:\n  name: a\n---\ntab:\n  name: b\n"), None);
    }
}

use std::fmt::Write as _;
use std::io::{self, IsTerminal, Write};

use ratatui::text::Line;
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Display {
    pub(crate) terminal: bool,
    color: bool,
    width: usize,
}

impl Display {
    pub(crate) fn stdout() -> Self {
        let terminal = io::stdout().is_terminal();
        Self {
            terminal,
            color: terminal
                && std::env::var_os("NO_COLOR").is_none_or(|value| value.is_empty())
                && std::env::var_os("TERM").is_none_or(|value| value != "dumb"),
            width: rustix::termios::tcgetwinsize(io::stdout())
                .ok()
                .map(|size| usize::from(size.ws_col))
                .filter(|width| *width > 0)
                .unwrap_or(80),
        }
    }

    fn heading(self, text: &str) -> String {
        if self.color {
            format!("\x1b[1;36m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }

    pub(crate) fn help(self, text: &str) -> String {
        let mut output = String::new();
        for line in text.lines() {
            let heading = line.ends_with(':') && !line.starts_with(' ');
            if heading || line.starts_with("Usage:") {
                let _ = writeln!(output, "{}", self.heading(line));
            } else if !line.starts_with(' ') && line.len() > self.width.min(100) {
                let mut length = 0;
                for word in line.split_whitespace() {
                    if length > 0 && length + 1 + word.len() > self.width.clamp(40, 100) {
                        output.push('\n');
                        length = 0;
                    }
                    if length > 0 {
                        output.push(' ');
                        length += 1;
                    }
                    output.push_str(word);
                    length += word.len();
                }
                output.push('\n');
            } else {
                let _ = writeln!(output, "{line}");
            }
        }
        output
    }

    pub(crate) fn table(self, headers: &[&str], rows: &[Vec<String>], empty: &str) -> String {
        if rows.is_empty() {
            return format!("{empty}\n");
        }
        let rows: Vec<Vec<String>> = rows
            .iter()
            .map(|row| row.iter().map(|cell| escape(cell)).collect())
            .collect();
        let widths: Vec<_> = headers
            .iter()
            .enumerate()
            .map(|(index, header)| {
                rows.iter()
                    .filter_map(|row| row.get(index))
                    .map(|cell| Line::raw(cell.as_str()).width())
                    .chain([header.len()])
                    .max()
                    .unwrap_or_default()
            })
            .collect();
        if widths.iter().sum::<usize>() + headers.len().saturating_sub(1) * 2 > self.width {
            return rows
                .iter()
                .map(|row| self.fields(headers, row))
                .collect::<Vec<_>>()
                .join("\n");
        }
        let mut output = String::new();
        let header: Vec<_> = headers.iter().map(|header| header.to_uppercase()).collect();
        output.push_str(&self.heading(&aligned(&header, &widths)));
        output.push('\n');
        for row in rows {
            output.push_str(&aligned(&row, &widths));
            output.push('\n');
        }
        output
    }

    pub(crate) fn details(self, value: &Value, columns: &[&str]) -> String {
        let fields: Vec<_> = columns.iter().map(|column| cell(&value[column])).collect();
        self.fields(columns, &fields)
    }

    fn fields(self, names: &[&str], values: &[String]) -> String {
        let width = names.iter().map(|name| name.len()).max().unwrap_or(0);
        let mut output = String::new();
        for (name, value) in names.iter().zip(values) {
            let name = format!("{name:width$}");
            let _ = writeln!(output, "{}  {value}", self.heading(&name));
        }
        output
    }
}

fn aligned(values: &[String], widths: &[usize]) -> String {
    let mut row = String::new();
    for (index, (value, width)) in values.iter().zip(widths).enumerate() {
        row.push_str(value);
        if index + 1 < values.len() {
            row.push_str(&" ".repeat(width.saturating_sub(Line::raw(value.as_str()).width()) + 2));
        }
    }
    row
}

pub(crate) fn cell(value: &Value) -> String {
    match value {
        Value::String(text) => escape(text),
        Value::Null => "-".into(),
        other => other.to_string(),
    }
}

fn escape(text: &str) -> String {
    text.chars()
        .flat_map(|character| {
            if character.is_control() {
                character.escape_default().collect::<Vec<_>>()
            } else {
                vec![character]
            }
        })
        .collect()
}

pub(crate) fn help(text: &str) -> io::Result<()> {
    write!(io::stdout(), "{}", Display::stdout().help(text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn display(width: usize) -> Display {
        Display {
            terminal: true,
            color: false,
            width,
        }
    }

    #[test]
    fn tables_have_headers_and_align_unicode_without_trailing_padding() {
        let rows = vec![
            vec!["界".into(), "one".into()],
            vec!["abc".into(), "two".into()],
        ];
        assert_eq!(
            display(80).table(&["id", "name"], &rows, "Nothing here."),
            "ID   NAME\n界   one\nabc  two\n"
        );
    }

    #[test]
    fn narrow_terminals_keep_full_values_and_empty_tables_explain_themselves() {
        assert_eq!(
            display(8).table(
                &["id", "name"],
                &[vec!["123456789".into(), "build".into()]],
                "None."
            ),
            "id    123456789\nname  build\n"
        );
        assert_eq!(
            display(80).table(&["id"], &[], "No sessions."),
            "No sessions.\n"
        );
    }

    #[test]
    fn terminal_values_cannot_inject_control_sequences() {
        let text = display(80).details(&json!({"name": "build\n\u{1b}[2J\tjob"}), &["name"]);
        assert_eq!(text, "name  build\\n\\u{1b}[2J\\tjob\n");
        assert!(!text.contains('\u{1b}'));
        assert_eq!(cell(&json!("it's a build")), "it's a build");
    }

    #[test]
    fn help_is_plain_unless_color_is_enabled() {
        let text = "Usage: muxy ls\n\nOptions:\n  --json  JSON output\n";
        assert_eq!(display(80).help(text), text);
        let colored = Display {
            color: true,
            ..display(80)
        }
        .help(text);
        assert!(colored.contains("\x1b[1;36mOptions:\x1b[0m"));
    }
}

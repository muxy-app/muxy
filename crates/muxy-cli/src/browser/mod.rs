pub(crate) mod args;

use std::error::Error;
use std::io::{self, Write};
use std::path::Path;

use serde_json::Value;

use crate::app::call_app;
use args::Output;
pub(crate) use args::{Invocation, parse};

const INVALID_REPLY: &str = "invalid reply from Muxy";

pub(crate) fn run(invocation: Invocation) -> Result<(), Box<dyn Error>> {
    let starts_app = invocation.verb == "browser.open";
    let reply = call_app(
        invocation.verb,
        invocation.args,
        starts_app,
        invocation.extra_time,
    )?;
    print(invocation.output, invocation.out.as_deref(), reply)
}

fn print(output: Output, out: Option<&Path>, reply: Value) -> Result<(), Box<dyn Error>> {
    let mut stdout = io::stdout().lock();
    match output {
        Output::Ok => writeln!(stdout, "ok")?,
        Output::Json => writeln!(stdout, "{reply}")?,
        Output::Text => {
            let text = match reply {
                Value::String(text) => text,
                other => other.to_string(),
            };
            if crate::output::Display::stdout().terminal {
                writeln!(stdout, "{}", crate::output::terminal_text(&text))?;
            } else {
                writeln!(stdout, "{text}")?;
            }
        }
        Output::Tabs => {
            let display = crate::output::Display::stdout();
            let columns = ["tabId", "title", "url", "profile", "active"];
            let rows: Vec<Vec<_>> = reply
                .as_array()
                .ok_or(INVALID_REPLY)?
                .iter()
                .map(|tab| {
                    columns
                        .iter()
                        .map(|key| {
                            if display.terminal {
                                crate::output::cell(&tab[key])
                            } else {
                                cell(&tab[key])
                            }
                        })
                        .collect()
                })
                .collect();
            if display.terminal {
                write!(
                    stdout,
                    "{}",
                    display.table(&columns, &rows, "No browser tabs.")
                )?;
            } else {
                for row in rows {
                    writeln!(stdout, "{}", row.join("\t"))?;
                }
            }
        }
        Output::Page => write!(stdout, "{}", page(&reply))?,
        Output::Screenshot => {
            let png = reply["png"].as_str().ok_or(INVALID_REPLY)?;
            match out {
                Some(file) => {
                    let bytes = muxy_core::base64::decode(png).ok_or(INVALID_REPLY)?;
                    std::fs::write(file, bytes)?;
                    writeln!(stdout, "{}", file.display())?;
                }
                None => writeln!(stdout, "{png}")?,
            }
        }
    }
    Ok(())
}

fn page(reply: &Value) -> String {
    format!(
        "{}\n{}\n{}\n",
        crate::output::cell(&Value::String(text(&reply["title"]).to_owned())),
        crate::output::cell(&Value::String(text(&reply["url"]).to_owned())),
        crate::output::terminal_text(text(&reply["text"])),
    )
}

fn cell(value: &Value) -> String {
    match value {
        Value::String(text) => crate::output::terminal_text(&text.replace(['\t', '\n', '\r'], " ")),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn page_reads_show_page_controlled_escapes_instead_of_sending_them() {
        let reply = json!({
            "title": "\u{1b}]52;c;aGk=\u{7}Title\nfake line",
            "url": "https://example.com/\u{1b}[2J",
            "text": "first line\n\tindented\u{1b}[31m red",
        });
        let output = page(&reply);
        assert!(!output.contains('\u{1b}'), "{output:?}");
        assert!(!output.contains('\u{7}'), "{output:?}");
        assert_eq!(
            output,
            "\\u{1b}]52;c;aGk=\\u{7}Title\\nfake line\nhttps://example.com/\\u{1b}[2J\n\
             first line\n\tindented\\u{1b}[31m red\n"
        );
        assert_eq!(page(&json!({})), "\n\n\n");
        assert_eq!(cell(&json!("a\tb\u{1b}c")), "a b\\u{1b}c");
    }
}

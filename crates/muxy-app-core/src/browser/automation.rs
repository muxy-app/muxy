//! The page side of browser automation. Each verb becomes the body of an
//! async JavaScript function that runs in the page; what it returns is the
//! verb's result.

use std::time::Duration;

use serde_json::{Map, Value};

pub type Args = Map<String, Value>;

/// Whether some JavaScript parses as the body of an async function.
pub type Parses = fn(&str) -> bool;

pub const DEFAULT_WAIT: Duration = Duration::from_millis(5000);
pub const DEFAULT_NAVIGATION_WAIT: Duration = Duration::from_millis(10_000);
pub const MAX_WAIT: Duration = Duration::from_millis(60_000);
pub const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub fn string<'a>(args: &'a Args, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing argument '{key}'"))
}

pub fn optional_string<'a>(args: &'a Args, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

pub fn flag(args: &Args, key: &str) -> bool {
    args.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// The `timeoutMs` argument, within what a command may wait.
pub fn timeout(args: &Args, default: Duration) -> Duration {
    args.get("timeoutMs")
        .and_then(Value::as_u64)
        .map_or(default, Duration::from_millis)
        .min(MAX_WAIT)
}

/// The script for a verb that runs in the page, or `None` when the app
/// handles `verb` itself.
pub fn script(verb: &str, args: &Args, parses: Parses) -> Result<Option<String>, String> {
    let element = |body: &str| -> Result<String, String> {
        Ok(format!(
            "{HELPERS}const el = document.querySelector({});\nif (!el) return false;\n{body}\nreturn true;",
            literal(string(args, "selector")?)
        ))
    };
    let value = |body: &str| -> Result<String, String> {
        Ok(format!(
            "const el = document.querySelector({});\nreturn el ? {body} : null;",
            literal(string(args, "selector")?)
        ))
    };
    let body = match verb {
        "eval" => function_body(string(args, "script")?, parses),
        "read" => "return { title: document.title, url: location.href, text: (document.body ? document.body.innerText : '').slice(0, 1000000) };".into(),
        "click" => element("el.click();")?,
        "fill" => element(&format!("el.focus();\nsetValue(el, {});", literal(string(args, "text")?)))?,
        "type" => element(&format!(
            "el.focus();\nsetValue(el, {});\nif ({}) submit(el);",
            literal(string(args, "text")?),
            flag(args, "submit")
        ))?,
        "select" => element(&format!("setValue(el, {});", literal(string(args, "value")?)))?,
        "hover" => element("for (const type of ['mouseover', 'mouseenter', 'mousemove']) el.dispatchEvent(new MouseEvent(type, { bubbles: type !== 'mouseenter', cancelable: true, view: window }));")?,
        "scrollIntoView" => element("el.scrollIntoView({ block: 'center', inline: 'center' });")?,
        "setChecked" => element(&format!("if (el.checked !== {}) el.click();", flag(args, "checked")))?,
        "press" => format!(
            "{HELPERS}const el = {};\nif (!el) return false;\npressKey(el, {});\nreturn true;",
            optional_string(args, "selector").map_or_else(
                || "document.activeElement || document.body".to_owned(),
                |selector| format!("document.querySelector({})", literal(selector))
            ),
            literal(string(args, "key")?)
        ),
        "is" => format!(
            "{HELPERS}const el = document.querySelector({});\nif (!el) return false;\n{}",
            literal(string(args, "selector")?),
            match string(args, "property")? {
                "checked" => "return Boolean(el.checked);",
                "enabled" => "return !el.disabled;",
                "disabled" => "return Boolean(el.disabled);",
                "hidden" => "return !visible(el);",
                _ => "return visible(el);",
            }
        ),
        "getText" => value("el.innerText")?,
        "getValue" => value("(el.value ?? null)")?,
        "getAttribute" => value(&format!("el.getAttribute({})", literal(string(args, "attribute")?)))?,
        "getHTML" => match optional_string(args, "selector") {
            Some(_) => value("el.outerHTML.slice(0, 5000000)")?,
            None => "return document.documentElement.outerHTML.slice(0, 5000000);".into(),
        },
        "getCount" => format!(
            "return document.querySelectorAll({}).length;",
            literal(string(args, "selector")?)
        ),
        "find" => format!(
            "{HELPERS}return find({}, {});",
            literal(optional_string(args, "kind").unwrap_or("text")),
            literal(string(args, "value")?)
        ),
        "snapshot" => format!(
            "{HELPERS}return snapshot({});",
            optional_string(args, "selector").map_or_else(|| "null".to_owned(), literal)
        ),
        "storage.get" => format!(
            "return {}.getItem({});",
            storage(args)?,
            literal(string(args, "key")?)
        ),
        "storage.set" => format!(
            "{}.setItem({}, {});\nreturn null;",
            storage(args)?,
            literal(string(args, "key")?),
            literal(string(args, "value")?)
        ),
        "storage.clear" => format!("{}.clear();\nreturn null;", storage(args)?),
        _ => return Ok(None),
    };
    Ok(Some(body))
}

/// A check for `wait` and `waitFor` that returns whether the condition
/// holds. Errors count as not yet.
pub fn condition(verb: &str, args: &Args, parses: Parses) -> Result<String, String> {
    let check = match verb {
        "waitFor" => format!(
            "return document.querySelector({}) !== null;",
            literal(string(args, "selector")?)
        ),
        "wait" => {
            if let Some(function) = optional_string(args, "function") {
                format!(
                    "return Boolean(await (async () => {{ {} }})());",
                    function_body(function, parses)
                )
            } else if let Some(selector) = optional_string(args, "selector") {
                format!(
                    "return document.querySelector({}) !== null;",
                    literal(selector)
                )
            } else if let Some(text) = optional_string(args, "text") {
                format!(
                    "return (document.body ? document.body.innerText : '').includes({});",
                    literal(text)
                )
            } else if let Some(fragment) = optional_string(args, "urlContains") {
                format!("return location.href.includes({});", literal(fragment))
            } else {
                "return true;".into()
            }
        }
        _ => return Err(format!("unknown command browser.{verb}")),
    };
    Ok(format!(
        "try {{\n{check}\n}} catch (error) {{\nreturn false;\n}}"
    ))
}

/// A script given on the command line: a single expression whose value is
/// the result, or a statement list that returns.
fn function_body(script: &str, parses: Parses) -> String {
    let statements = script.trim();
    let expression = format!("return (\n{}\n);", statements.trim_end_matches(';'));
    if parses(&expression) {
        expression
    } else {
        statements.to_owned()
    }
}

fn storage(args: &Args) -> Result<&'static str, String> {
    match optional_string(args, "kind").unwrap_or("local") {
        "local" => Ok("window.localStorage"),
        "session" => Ok("window.sessionStorage"),
        other => Err(format!("unknown storage '{other}'; use local or session")),
    }
}

/// `value` as a JavaScript string literal.
pub fn literal(value: &str) -> String {
    Value::from(value)
        .to_string()
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

const HELPERS: &str = r#"const setValue = (el, value) => {
  const prototype = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype
    : el instanceof HTMLSelectElement ? HTMLSelectElement.prototype
    : el instanceof HTMLInputElement ? HTMLInputElement.prototype : null;
  const setter = prototype && Object.getOwnPropertyDescriptor(prototype, 'value')?.set;
  if (setter) setter.call(el, value);
  else if (el.isContentEditable) el.textContent = value;
  else el.value = value;
  el.dispatchEvent(new Event('input', { bubbles: true }));
  el.dispatchEvent(new Event('change', { bubbles: true }));
};
const pressKey = (el, key) => {
  for (const type of ['keydown', 'keypress', 'keyup']) {
    el.dispatchEvent(new KeyboardEvent(type, { key, bubbles: true, cancelable: true }));
  }
};
const submit = el => {
  const form = el.form || el.closest('form');
  if (!form) pressKey(el, 'Enter');
  else if (form.requestSubmit) form.requestSubmit();
  else form.submit();
};
const visible = el => {
  const box = el.getBoundingClientRect();
  const style = getComputedStyle(el);
  return box.width > 0 && box.height > 0 && style.visibility !== 'hidden' && style.display !== 'none';
};
const label = el => (el.innerText || el.value || el.getAttribute('aria-label') || el.getAttribute('placeholder') || '').trim().slice(0, 120);
const roleOf = el => {
  const explicit = el.getAttribute('role');
  if (explicit) return explicit;
  const tag = el.tagName.toLowerCase();
  if (tag === 'a' && el.hasAttribute('href')) return 'link';
  if (tag === 'button' || tag === 'summary') return 'button';
  if (tag === 'select') return 'combobox';
  if (tag === 'textarea') return 'textbox';
  if (/^h[1-6]$/.test(tag)) return 'heading';
  if (tag === 'img') return 'img';
  if (tag === 'input') {
    const type = (el.getAttribute('type') || 'text').toLowerCase();
    if (['button', 'submit', 'reset', 'image'].includes(type)) return 'button';
    if (['checkbox', 'radio', 'range'].includes(type)) return type === 'range' ? 'slider' : type;
    return type === 'search' ? 'searchbox' : 'textbox';
  }
  return null;
};
const selectorOf = el => {
  if (el.id && document.querySelectorAll('#' + CSS.escape(el.id)).length === 1) return '#' + CSS.escape(el.id);
  const testid = el.getAttribute('data-testid');
  if (testid) {
    const selector = '[data-testid="' + CSS.escape(testid) + '"]';
    if (document.querySelectorAll(selector).length === 1) return selector;
  }
  const parts = [];
  for (let node = el; node && node.nodeType === 1 && node !== document.documentElement; node = node.parentElement) {
    const tag = node.tagName.toLowerCase();
    const siblings = node.parentElement ? Array.from(node.parentElement.children).filter(child => child.tagName === node.tagName) : [];
    parts.unshift(siblings.length > 1 ? tag + ':nth-of-type(' + (siblings.indexOf(node) + 1) + ')' : tag);
  }
  return parts.join(' > ');
};
const find = (kind, value) => {
  const needle = value.toLowerCase();
  const includes = text => (text || '').toLowerCase().includes(needle);
  const matches = {
    role: el => roleOf(el) === value,
    testid: el => el.getAttribute('data-testid') === value,
    label: el => includes(el.getAttribute('aria-label')) || Array.from(el.labels || []).some(item => includes(item.innerText)),
    placeholder: el => includes(el.getAttribute('placeholder')),
    text: el => includes(el.innerText),
  }[kind];
  if (!matches) throw new Error('unknown find kind ' + kind + '; use role, text, label, placeholder or testid');
  const found = Array.from(document.body ? document.body.querySelectorAll('*') : []).filter(matches);
  return found
    .filter(el => !found.some(other => other !== el && el.contains(other)))
    .slice(0, 20)
    .map(el => ({ tag: el.tagName.toLowerCase(), text: label(el), role: roleOf(el), id: el.id || null, testid: el.getAttribute('data-testid'), selector: selectorOf(el) }));
};
const snapshot = selector => {
  const root = selector ? document.querySelector(selector) : document.body;
  if (!root) return [];
  const interactive = 'a, button, input, textarea, select, summary, [role=button], [role=link], [role=tab], [role=checkbox], [role=menuitem], [onclick], [contenteditable=true]';
  return Array.from(root.querySelectorAll(interactive))
    .filter(visible)
    .slice(0, 200)
    .map(el => ({ tag: el.tagName.toLowerCase(), role: roleOf(el) || el.type || null, name: label(el), id: el.id || null, testid: el.getAttribute('data-testid'), selector: selectorOf(el), visible: true }));
};
"#;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn args(value: Value) -> Args {
        match value {
            Value::Object(map) => map,
            _ => Args::new(),
        }
    }

    fn parses_all(_: &str) -> bool {
        true
    }

    fn parses_none(_: &str) -> bool {
        false
    }

    #[test]
    fn parsed_expressions_return_their_value_and_other_bodies_run_as_given() -> Result<(), String> {
        let expression = script(
            "eval",
            &args(json!({"script": " document.title; "})),
            parses_all,
        )?;
        assert_eq!(expression.as_deref(), Some("return (\ndocument.title\n);"));
        let body = script(
            "eval",
            &args(json!({"script": " const a = 1; return a; "})),
            parses_none,
        )?;
        assert_eq!(body.as_deref(), Some("const a = 1; return a;"));
        Ok(())
    }

    #[test]
    fn arguments_are_embedded_as_literals() -> Result<(), String> {
        let script = script(
            "type",
            &args(
                json!({"selector": "input[name=\"q\"]", "text": "a'b\"c\u{2028}", "submit": true}),
            ),
            parses_none,
        )?
        .unwrap_or_default();
        assert!(script.contains(r#"document.querySelector("input[name=\"q\"]")"#));
        assert!(script.contains("setValue(el, \"a'b\\\"c\\u2028\");"));
        assert!(script.contains("if (true) submit(el);"));
        Ok(())
    }

    #[test]
    fn missing_arguments_are_reported() {
        assert_eq!(
            script("click", &Args::new(), parses_none),
            Err("missing argument 'selector'".to_owned())
        );
        assert_eq!(
            script(
                "storage.get",
                &args(json!({"key": "a", "kind": "disk"})),
                parses_none
            ),
            Err("unknown storage 'disk'; use local or session".to_owned())
        );
    }

    #[test]
    fn app_verbs_have_no_page_script() -> Result<(), String> {
        for verb in ["open", "navigate", "screenshot", "cookies.get", "reload"] {
            assert_eq!(script(verb, &Args::new(), parses_none)?, None, "{verb}");
        }
        Ok(())
    }

    #[test]
    fn wait_conditions_follow_their_priority_and_swallow_errors() -> Result<(), String> {
        let function = condition(
            "wait",
            &args(json!({"function": "window.ready", "selector": "#a"})),
            parses_all,
        )?;
        assert!(function.contains("return (\nwindow.ready\n);"));
        assert!(function.contains("catch (error)"));
        let text = condition(
            "wait",
            &args(json!({"text": "Done", "urlContains": "x"})),
            parses_none,
        )?;
        assert!(text.contains(r#".includes("Done")"#));
        assert!(condition("wait", &Args::new(), parses_none)?.contains("return true;"));
        assert!(condition("waitFor", &Args::new(), parses_none).is_err());
        Ok(())
    }

    #[test]
    fn timeouts_are_capped() {
        assert_eq!(timeout(&Args::new(), DEFAULT_WAIT), DEFAULT_WAIT);
        assert_eq!(
            timeout(&args(json!({"timeoutMs": 900_000})), DEFAULT_WAIT),
            MAX_WAIT
        );
    }
}

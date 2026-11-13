//! What `muxy extension page` asks of an extension page: scripts whose result
//! comes back through the page's message handler, and pictures of the page.

use std::collections::HashMap;

use async_channel::{Receiver, Sender};
use muxy_app_core::browser::automation::literal;
use muxy_ui::webview::Snapshot;
use serde_json::Value;

/// The message a page posts with what a script returned.
pub(crate) const RESULT_VERB: &str = "automation.result";
/// Keeps an answer under the size of message a page may post.
const RESULT_LIMIT: usize = 900_000;

pub(crate) type Outcome = Result<Value, String>;

/// A page as it looked, in the premultiplied BGRA its snapshot came in.
pub(crate) struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Default)]
pub(crate) struct Automation {
    next: u64,
    scripts: HashMap<String, Sender<Outcome>>,
    captures: Vec<(u64, Sender<Result<Picture, String>>)>,
}

impl Automation {
    /// The script that runs `body` as an async function and posts its
    /// result, the token that result comes back with, and where it arrives.
    pub(crate) fn start(&mut self, body: &str) -> (String, String, Receiver<Outcome>) {
        self.next += 1;
        let token = format!("automation-{}", self.next);
        let (sender, receiver) = async_channel::bounded(1);
        self.scripts.insert(token.clone(), sender);
        (wrap(&token, body), token, receiver)
    }

    /// Stops waiting for a script that ran out of time.
    pub(crate) fn forget(&mut self, token: &str) {
        self.scripts.remove(token);
    }

    /// Hands a posted result to the script that is waiting for it. Whether
    /// one was.
    pub(crate) fn finish(&mut self, token: &str, args: &Value) -> bool {
        let Some(waiting) = self.scripts.remove(token) else {
            return false;
        };
        let outcome = if args["ok"].as_bool() == Some(true) {
            args["json"]
                .as_str()
                .ok_or_else(|| "the page answered without a result".to_owned())
                .and_then(|json| serde_json::from_str(json).map_err(|error| error.to_string()))
        } else {
            Err(args["error"]
                .as_str()
                .unwrap_or("the script failed")
                .to_owned())
        };
        let _ = waiting.try_send(outcome);
        true
    }

    /// Waits for the next snapshot. The number forgets the wait.
    pub(crate) fn capture(&mut self) -> (u64, Receiver<Result<Picture, String>>) {
        self.next += 1;
        let (sender, receiver) = async_channel::bounded(1);
        self.captures.push((self.next, sender));
        (self.next, receiver)
    }

    /// Stops waiting for a snapshot that didn't come in time.
    pub(crate) fn forget_capture(&mut self, id: u64) {
        self.captures.retain(|(capture, _)| *capture != id);
    }

    pub(crate) fn wants_capture(&self) -> bool {
        !self.captures.is_empty()
    }

    pub(crate) fn deliver(&mut self, snapshot: &Snapshot) {
        for (_, waiting) in self.captures.drain(..) {
            let _ = waiting.try_send(Ok(Picture {
                width: snapshot.width,
                height: snapshot.height,
                rgba: snapshot.bgra.clone(),
            }));
        }
    }

    pub(crate) fn fail_captures(&mut self, error: &str) {
        for (_, waiting) in self.captures.drain(..) {
            let _ = waiting.try_send(Err(error.to_owned()));
        }
    }

    /// A page that shows another document never answers what the last one
    /// was asked.
    pub(crate) fn fail_scripts(&mut self, error: &str) {
        for (_, waiting) in self.scripts.drain() {
            let _ = waiting.try_send(Err(error.to_owned()));
        }
    }
}

fn wrap(token: &str, body: &str) -> String {
    format!(
        "(() => {{\n\
         const handler = window.webkit?.messageHandlers?.muxy;\n\
         if (!handler) return;\n\
         const post = args => Promise.resolve().then(() => handler.postMessage({{ verb: {verb}, requestID: {token}, args }}));\n\
         const fail = error => post({{ ok: false, error: String((error && error.message) || error) }}).catch(() => {{}});\n\
         const tooLarge = 'the result is too large; ask for less of the page';\n\
         (async () => {{\n{body}\n}})().then(value => {{\n\
         const json = JSON.stringify(value === undefined ? null : value) ?? 'null';\n\
         const message = {{ ok: true, json }};\n\
         if (new TextEncoder().encode(JSON.stringify(message)).length > {RESULT_LIMIT}) return fail(tooLarge);\n\
         return post(message).catch(() => fail(tooLarge));\n\
         }}).catch(fail);\n\
         }})();",
        verb = literal(RESULT_VERB),
        token = literal(token),
    )
}

/// Turns the premultiplied BGRA of a snapshot into PNG bytes.
pub(crate) fn png(picture: Picture) -> Result<Vec<u8>, String> {
    let Picture {
        width,
        height,
        mut rgba,
    } = picture;
    super::unpremultiply(&mut rgba);
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or("the screenshot has the wrong size")?;
    let mut encoded = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(encoded.into_inner())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_posted_result_reaches_the_script_waiting_for_it() {
        let mut automation = Automation::default();
        let (script, token, result) = automation.start("return 1;");
        assert_eq!(token, "automation-1");
        assert!(script.contains("\"automation-1\""), "{script}");
        assert!(script.contains("return 1;"), "{script}");
        assert!(!automation.finish("automation-9", &json!({"ok": true, "json": "2"})));
        assert!(automation.finish("automation-1", &json!({"ok": true, "json": "{\"a\":1}"})));
        assert_eq!(result.try_recv().ok(), Some(Ok(json!({"a": 1}))));
        assert!(!automation.finish("automation-1", &json!({"ok": true, "json": "1"})));
    }

    #[test]
    fn a_failed_script_reports_its_error() {
        let mut automation = Automation::default();
        let (_, _, result) = automation.start("throw new Error('no');");
        automation.finish("automation-1", &json!({"ok": false, "error": "no"}));
        assert_eq!(result.try_recv().ok(), Some(Err("no".to_owned())));
        let (_, token, forgotten) = automation.start("return 2;");
        automation.forget(&token);
        assert!(forgotten.is_closed(), "a forgotten script stops waiting");
        let (_, _, result) = automation.start("return 1;");
        automation.fail_scripts("the page navigated");
        assert_eq!(
            result.try_recv().ok(),
            Some(Err("the page navigated".to_owned()))
        );
    }

    #[test]
    fn captures_wait_for_the_next_snapshot() {
        let mut automation = Automation::default();
        let (forgotten, unwanted) = automation.capture();
        automation.forget_capture(forgotten);
        assert!(unwanted.is_closed(), "a forgotten capture stops waiting");
        assert!(!automation.wants_capture());
        let (_, picture) = automation.capture();
        assert!(automation.wants_capture());
        automation.deliver(&Snapshot {
            width: 1,
            height: 1,
            bgra: vec![0, 0, 128, 128],
        });
        assert!(!automation.wants_capture());
        let picture = picture.try_recv().ok().and_then(Result::ok);
        let png = picture.map(png).and_then(Result::ok).unwrap_or_default();
        let decoded = image::load_from_memory(&png).map(|image| image.to_rgba8());
        assert_eq!(
            decoded.ok().map(image::ImageBuffer::into_raw),
            Some(vec![255, 0, 0, 128])
        );
    }
}

pub(super) const SCHEME: &str = "muxy-ext";
pub(super) const HANDLER: &str = "muxy";

pub(super) const CONTENT_SECURITY_POLICY: &str = "default-src 'self' muxy-ext: data: blob:; \
     script-src 'self' muxy-ext: data: blob: 'unsafe-inline' 'unsafe-eval'; \
     style-src 'self' muxy-ext: data: blob: 'unsafe-inline'";

const CLICK_EVIDENCE: std::time::Duration = std::time::Duration::from_secs(2);

pub(super) fn take_click(last: &std::cell::Cell<Option<std::time::Instant>>) -> bool {
    last.take()
        .is_some_and(|clicked| clicked.elapsed() <= CLICK_EVIDENCE)
}

const MAX_MESSAGE_BYTES: usize = 1024 * 1024;
const MAX_PENDING_REPLIES: usize = 128;
const MAX_PENDING_ASSETS: usize = 32;

pub(super) const NOT_MAIN_DOCUMENT: &str = "message is not from the registered main document";
pub(super) const INVALID_MESSAGE: &str = "invalid JSON message";
pub(super) const REQUEST_LIMIT: &str = "webview request limit exceeded";
pub(super) const QUEUE_FULL: &str = "webview request queue full";
pub(super) const DOCUMENT_RETIRED: &str = "document retired";
pub(super) const INVALID_RESPONSE: &str = "invalid host response";

pub(super) fn owns(scheme: &str, host: &str, owner: &str) -> bool {
    scheme == SCHEME && host == owner
}

pub(super) fn may_navigate(scheme: &str, host: &str, owner: &str) -> bool {
    owns(scheme, host, owner) || scheme == "about"
}

pub(super) fn admits_message(bytes: usize, pending: usize) -> bool {
    bytes <= MAX_MESSAGE_BYTES && pending < MAX_PENDING_REPLIES
}

pub(super) fn admits_asset(pending: usize) -> bool {
    pending < MAX_PENDING_ASSETS
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn origin(url: &str) -> Option<(&str, &str)> {
    let (scheme, rest) = url.split_once(':')?;
    let host = rest
        .strip_prefix("//")
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .unwrap_or("");
    Some((scheme, host))
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn path(url: &str) -> Option<String> {
    let rest = url.split_once("://")?.1;
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let path = rest.find('/').map_or("", |start| &rest[start..]);
    let mut bytes = Vec::with_capacity(path.len());
    let mut input = path.bytes();
    while let Some(byte) = input.next() {
        if byte == b'%' {
            let high = char::from(input.next()?).to_digit(16)?;
            let low = char::from(input.next()?).to_digit(16)?;
            bytes.push(u8::try_from(high * 16 + low).ok()?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_moves_focus_once_and_only_while_recent() {
        use std::cell::Cell;
        use std::time::{Duration, Instant};
        let last = Cell::new(None);
        assert!(!take_click(&last));
        last.set(Some(Instant::now()));
        assert!(take_click(&last));
        assert!(!take_click(&last));
        last.set(Instant::now().checked_sub(Duration::from_secs(5)));
        assert!(!take_click(&last));
    }

    #[test]
    fn the_content_policy_names_no_remote_source() {
        let policy = CONTENT_SECURITY_POLICY;
        assert!(policy.starts_with("default-src 'self' muxy-ext: data: blob:;"));
        for directive in policy.split(';') {
            let sources = directive.split_whitespace().skip(1);
            for source in sources {
                assert!(
                    source.starts_with('\'') || ["muxy-ext:", "data:", "blob:"].contains(&source),
                    "{source} reaches the network",
                );
            }
        }
    }

    #[test]
    fn only_the_owners_own_pages_and_blank_documents_are_reachable() {
        let allowed = |url: &str| {
            origin(url).is_some_and(|(scheme, host)| may_navigate(scheme, host, "git.tools"))
        };
        assert!(allowed("muxy-ext://git.tools/index.html"));
        assert!(allowed("muxy-ext://git.tools"));
        assert!(allowed("about:blank"));
        assert!(allowed("about:srcdoc"));
        for url in [
            "muxy-ext://other/index.html",
            "muxy-ext://git.tools@other/index.html",
            "muxy-ext://git.tools:80/index.html",
            "muxy-ext://git.tools.other/index.html",
            "muxy-ext:/git.tools/index.html",
            "https://git.tools/index.html",
            "file:///etc/passwd",
            "data:text/html,hello",
            "blob:muxy-ext://git.tools/1",
            "javascript:alert(1)",
            "git.tools/index.html",
        ] {
            assert!(!allowed(url), "{url}");
        }
    }

    #[test]
    fn asset_paths_are_decoded_once_and_stop_at_the_query() {
        assert_eq!(
            path("muxy-ext://owner/a%20b/c.js?x=/1#y").as_deref(),
            Some("/a b/c.js")
        );
        assert_eq!(
            path("muxy-ext://owner/%2e%2e/secret").as_deref(),
            Some("/../secret")
        );
        assert_eq!(path("muxy-ext://owner/%2541").as_deref(), Some("/%41"));
        assert_eq!(path("muxy-ext://owner").as_deref(), Some(""));
        assert_eq!(path("muxy-ext://owner?x=/y").as_deref(), Some(""));
        assert_eq!(path("muxy-ext://owner/%zz"), None);
        assert_eq!(path("muxy-ext://owner/%ff"), None);
        assert_eq!(path("muxy-ext://owner/%4"), None);
    }

    #[test]
    fn requests_and_assets_are_bounded() {
        assert!(admits_message(MAX_MESSAGE_BYTES, MAX_PENDING_REPLIES - 1));
        assert!(!admits_message(MAX_MESSAGE_BYTES + 1, 0));
        assert!(!admits_message(1, MAX_PENDING_REPLIES));
        assert!(admits_asset(MAX_PENDING_ASSETS - 1));
        assert!(!admits_asset(MAX_PENDING_ASSETS));
    }
}

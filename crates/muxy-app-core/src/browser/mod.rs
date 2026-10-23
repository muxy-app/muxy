//! The built-in browser's model: what a browser pane saves, how typed text
//! becomes an address, and how the CLI talks to the app about it.

pub mod api;
pub mod automation;
mod target;

use std::fmt::Write;

use serde::{Deserialize, Serialize};

use crate::{AppError, AppState, Direction, Pane, PaneContent, PaneId, ProjectId, Tab, TabId};

pub use target::resolve_target;

/// The page a pane with no address shows: the start page.
pub const BLANK: &str = "about:blank";
pub const MIN_ZOOM: f64 = 0.5;
pub const MAX_ZOOM: f64 = 3.0;
const ZOOM_STEP: f64 = 1.1;

/// What a browser pane keeps across restarts. Title, zoom and back/forward
/// history belong to the live page and are not saved.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct BrowserPane {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The browser profile's ID; none means the default profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

impl BrowserPane {
    pub fn new(url: Option<String>, profile: Option<String>) -> Self {
        Self {
            url: url.filter(|url| !is_blank(url)),
            profile,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchEngine {
    #[default]
    Google,
    DuckDuckGo,
    Bing,
    Brave,
    Startpage,
}

impl SearchEngine {
    pub const ALL: [Self; 5] = [
        Self::Google,
        Self::DuckDuckGo,
        Self::Bing,
        Self::Brave,
        Self::Startpage,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Bing => "Bing",
            Self::Brave => "Brave Search",
            Self::Startpage => "Startpage",
        }
    }

    const fn endpoint(self) -> &'static str {
        match self {
            Self::Google => "https://www.google.com/search",
            Self::DuckDuckGo => "https://duckduckgo.com/",
            Self::Bing => "https://www.bing.com/search",
            Self::Brave => "https://search.brave.com/search",
            Self::Startpage => "https://www.startpage.com/sp/search",
        }
    }

    pub fn search_url(self, query: &str) -> String {
        format!("{}?q={}", self.endpoint(), encode_query(query))
    }
}

pub fn is_blank(url: &str) -> bool {
    url.is_empty() || url == BLANK
}

/// Turns what was typed in the address bar into an address, or else a
/// search. Schemes a page could abuse, such as `javascript:` or `file:`,
/// become searches.
pub fn resolve_input(input: &str, engine: SearchEngine) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    resolve_address(input).or_else(|| Some(engine.search_url(input)))
}

/// A web address as typed, or a host name with a scheme added.
pub fn resolve_address(input: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() || input.contains(char::is_whitespace) {
        return None;
    }
    if let Some((scheme, rest)) = input.split_once(':') {
        match scheme.to_ascii_lowercase().as_str() {
            "about" => return Some(input.into()),
            "http" | "https" if rest.starts_with("//") && rest.len() > 2 => {
                return Some(input.into());
            }
            _ => {}
        }
    }
    let host = host_of(input)?;
    let scheme = if is_local_host(host) { "http" } else { "https" };
    Some(format!("{scheme}://{input}"))
}

/// The host part of text that names a site, such as `example.com/a` or
/// `localhost:3000`.
fn host_of(input: &str) -> Option<&str> {
    let host = input
        .split(['/', '?', '#'])
        .next()
        .filter(|host| !host.is_empty())?;
    let name = host
        .rsplit_once(':')
        .filter(|(_, port)| !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()))
        .map_or(host, |(name, _)| name);
    let dotted = name.contains('.') && !name.starts_with('.') && !name.ends_with('.');
    let valid = name
        .chars()
        .all(|character| character.is_alphanumeric() || matches!(character, '.' | '-' | '_'));
    (valid && (dotted || name.eq_ignore_ascii_case("localhost"))).then_some(name)
}

/// Development servers rarely speak TLS, so these hosts get `http://`.
fn is_local_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.to_ascii_lowercase().ends_with(".localhost")
        || host
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|address| address.is_loopback() || address.is_private())
}

/// Whether a page may load `url` itself. Other schemes, such as `mailto:`
/// or `file:`, never load in a page; a click on one goes to the system.
pub fn engine_loads(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        ["http", "https", "about", "data", "blob"]
            .iter()
            .any(|allowed| scheme.eq_ignore_ascii_case(allowed))
    })
}

/// The host of an absolute address, for titles and cookie filters.
pub fn url_host(url: &str) -> Option<&str> {
    let (_, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = host
        .rsplit_once(':')
        .filter(|(_, port)| port.bytes().all(|byte| byte.is_ascii_digit()))
        .map_or(host, |(host, _)| host);
    (!host.is_empty()).then_some(host)
}

/// What a browser tab is called: the page title, else its host.
pub fn display_title<'a>(title: &'a str, url: Option<&'a str>) -> Option<&'a str> {
    let title = title.trim();
    if !title.is_empty() {
        return Some(title);
    }
    url.filter(|url| !is_blank(url)).and_then(url_host)
}

pub fn zoom_in(zoom: f64) -> f64 {
    (zoom * ZOOM_STEP).min(MAX_ZOOM)
}

pub fn zoom_out(zoom: f64) -> f64 {
    (zoom / ZOOM_STEP).max(MIN_ZOOM)
}

fn encode_query(query: &str) -> String {
    let mut encoded = String::with_capacity(query.len());
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(byte));
            }
            b' ' => encoded.push('+'),
            _ => {
                let _ = write!(encoded, "%{byte:02X}");
            }
        }
    }
    encoded
}

impl AppState {
    /// Opens a browser tab in `project`. With `select`, the window shows it;
    /// otherwise it waits there without moving the user's view.
    pub fn open_browser(
        &mut self,
        project: ProjectId,
        browser: BrowserPane,
        select: bool,
    ) -> Result<(TabId, PaneId), AppError> {
        let tab = Tab::with_content(PaneContent::Browser(browser), "New Tab");
        let id = tab.id;
        let pane = tab.panes[0].id;
        let near = self.window.selected_tab.get(&project).copied();
        self.add_tab(project, tab, near)?;
        if select {
            self.select_project(project)?;
            self.select_tab(project, id)?;
        } else {
            self.sync_groups(project);
        }
        Ok((id, pane))
    }

    /// Splits `pane` with a browser on its right.
    pub fn split_browser(
        &mut self,
        pane: PaneId,
        browser: BrowserPane,
        focus: bool,
    ) -> Result<PaneId, AppError> {
        let tab = self.pane_tab_mut(pane)?;
        let new = Pane {
            id: PaneId::new(),
            title: "New Tab".into(),
            content: PaneContent::Browser(browser),
            proxy: false,
        };
        tab.layout.split(pane, new.id, Direction::Right);
        let id = new.id;
        tab.zoomed = None;
        tab.panes.push(new);
        if focus {
            self.focus_pane(id)?;
        }
        Ok(id)
    }

    /// The project whose tab holds `pane`.
    pub fn pane_project(&self, pane: PaneId) -> Option<ProjectId> {
        self.projects()
            .iter()
            .find(|project| {
                project
                    .tabs
                    .iter()
                    .any(|tab| tab.panes.iter().any(|candidate| candidate.id == pane))
            })
            .map(|project| project.id)
    }

    pub fn browser(&self, pane: PaneId) -> Option<&BrowserPane> {
        self.projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .flat_map(|tab| &tab.panes)
            .find(|candidate| candidate.id == pane)
            .and_then(|pane| match &pane.content {
                PaneContent::Browser(browser) => Some(browser),
                _ => None,
            })
    }

    /// The browser pane `id` names: that pane, or the one a tab of that ID
    /// shows, else the tab's first browser pane.
    pub fn browser_pane(&self, id: &str) -> Option<PaneId> {
        if let Some(pane) = id.parse().ok().filter(|pane| self.browser(*pane).is_some()) {
            return Some(pane);
        }
        let tab: TabId = id.parse().ok()?;
        let tab = self
            .projects()
            .iter()
            .flat_map(|project| &project.tabs)
            .find(|candidate| candidate.id == tab)?;
        let is_browser = |pane: &PaneId| self.browser(*pane).is_some();
        self.shown_pane(tab)
            .filter(is_browser)
            .or_else(|| tab.layout.leaves().into_iter().find(is_browser))
    }

    /// Records the page a browser pane shows, reporting whether it changed.
    pub fn set_browser_url(&mut self, pane: PaneId, url: Option<String>) -> Result<bool, AppError> {
        let browser = self.browser_mut(pane)?;
        let url = url.filter(|url| !is_blank(url));
        let changed = browser.url != url;
        browser.url = url;
        Ok(changed)
    }

    pub fn set_browser_profile(
        &mut self,
        pane: PaneId,
        profile: Option<String>,
    ) -> Result<bool, AppError> {
        let browser = self.browser_mut(pane)?;
        let changed = browser.profile != profile;
        browser.profile = profile;
        Ok(changed)
    }

    fn browser_mut(&mut self, pane: PaneId) -> Result<&mut BrowserPane, AppError> {
        match &mut self.pane_mut(pane)?.content {
            PaneContent::Browser(browser) => Ok(browser),
            _ => Err(AppError::InvalidState("pane is not a browser".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(input: &str) -> Option<String> {
        resolve_input(input, SearchEngine::Google)
    }

    #[test]
    fn typed_text_becomes_an_address_or_a_search() {
        assert_eq!(
            resolve("https://example.com/a?b=c").as_deref(),
            Some("https://example.com/a?b=c")
        );
        assert_eq!(
            resolve("http://example.com").as_deref(),
            Some("http://example.com")
        );
        assert_eq!(
            resolve("example.com/docs").as_deref(),
            Some("https://example.com/docs")
        );
        assert_eq!(
            resolve("localhost:3000/app").as_deref(),
            Some("http://localhost:3000/app")
        );
        assert_eq!(
            resolve("127.0.0.1:8080").as_deref(),
            Some("http://127.0.0.1:8080")
        );
        assert_eq!(resolve("about:blank").as_deref(), Some("about:blank"));
        assert_eq!(
            resolve("rust lifetimes").as_deref(),
            Some("https://www.google.com/search?q=rust+lifetimes")
        );
        assert_eq!(
            resolve("muxy").as_deref(),
            Some("https://www.google.com/search?q=muxy")
        );
        assert_eq!(resolve("   "), None);
    }

    #[test]
    fn unsafe_schemes_become_searches() {
        for input in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "file:///etc/passwd",
            "vbscript:x",
        ] {
            assert!(
                resolve(input)
                    .is_some_and(|url| url.starts_with("https://www.google.com/search?q=")),
                "{input}"
            );
        }
    }

    #[test]
    fn searches_use_the_chosen_engine_and_encode_the_query() {
        assert_eq!(
            SearchEngine::DuckDuckGo.search_url("a&b=c ü"),
            "https://duckduckgo.com/?q=a%26b%3Dc+%C3%BC"
        );
        assert_eq!(
            resolve_input("cats", SearchEngine::Brave).as_deref(),
            Some("https://search.brave.com/search?q=cats")
        );
    }

    #[test]
    fn pages_load_only_web_schemes() {
        assert!(engine_loads("https://example.com"));
        assert!(engine_loads("about:blank"));
        assert!(engine_loads("blob:https://example.com/1"));
        assert!(!engine_loads("file:///etc/passwd"));
        assert!(!engine_loads("mailto:me@example.com"));
        assert!(!engine_loads("javascript:alert(1)"));
    }

    #[test]
    fn titles_fall_back_to_the_host() {
        assert_eq!(display_title(" Docs ", None), Some("Docs"));
        assert_eq!(
            display_title("", Some("https://user@docs.rs:443/x")),
            Some("docs.rs")
        );
        assert_eq!(display_title("", Some(BLANK)), None);
    }

    #[test]
    fn zoom_steps_are_clamped() {
        assert!((zoom_in(1.0) - 1.1).abs() < f64::EPSILON);
        assert!((zoom_in(2.9) - MAX_ZOOM).abs() < f64::EPSILON);
        assert!((zoom_out(0.52) - MIN_ZOOM).abs() < f64::EPSILON);
    }
}

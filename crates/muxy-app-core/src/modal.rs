use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

use regex::{Regex, RegexBuilder};

pub const MAX_ITEMS: usize = 100_000;
pub const MAX_FIELD_CHARACTERS: usize = 200;
static NEXT_MODAL: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModalItem {
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
}

impl ModalItem {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
        }
    }

    fn bounded(mut self) -> Option<Self> {
        self.id = bounded(&self.id);
        self.title = bounded(&self.title);
        self.subtitle = self.subtitle.map(|value| bounded(&value));
        (!self.id.is_empty() && !self.title.is_empty()).then_some(self)
    }
}

#[derive(Clone, Debug)]
pub struct ModalOptions {
    pub placeholder: String,
    pub empty_label: String,
    pub no_match_label: String,
    pub search_toolbar: bool,
    pub dynamic: bool,
}

impl Default for ModalOptions {
    fn default() -> Self {
        Self {
            placeholder: "Search…".into(),
            empty_label: "No items".into(),
            no_match_label: "No matches".into(),
            search_toolbar: false,
            dynamic: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModalToken {
    pub id: u64,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModalQuery {
    pub token: ModalToken,
    pub query: String,
    pub options: SearchOptions,
}

#[derive(Debug)]
pub struct ModalState {
    token: ModalToken,
    pub options: ModalOptions,
    query: String,
    search: SearchOptions,
    matcher: Option<Regex>,
    search_error: Option<String>,
    items: Vec<ModalItem>,
    ids: HashSet<String>,
    loading: bool,
    completed: bool,
    querying: bool,
}

impl ModalState {
    pub fn new(mut options: ModalOptions, items: Vec<ModalItem>, streaming: bool) -> Self {
        options.placeholder = bounded(&options.placeholder);
        options.empty_label = bounded(&options.empty_label);
        options.no_match_label = bounded(&options.no_match_label);
        let mut state = Self {
            token: ModalToken {
                id: NEXT_MODAL.fetch_add(1, Ordering::Relaxed),
                revision: 0,
            },
            options,
            query: String::new(),
            search: SearchOptions::default(),
            matcher: None,
            search_error: None,
            items: Vec::new(),
            ids: HashSet::new(),
            loading: true,
            completed: false,
            querying: false,
        };
        state.feed(state.token, items);
        if !streaming {
            state.finish(state.token);
        }
        state
    }

    pub fn token(&self) -> ModalToken {
        self.token
    }
    pub fn loading(&self) -> bool {
        self.loading
    }
    pub fn query(&self) -> &str {
        &self.query
    }
    pub fn search_options(&self) -> SearchOptions {
        self.search
    }
    pub fn search_error(&self) -> Option<&str> {
        self.search_error.as_deref()
    }

    pub fn feed(&mut self, token: ModalToken, items: impl IntoIterator<Item = ModalItem>) -> bool {
        if token != self.token || self.completed || !self.loading {
            return false;
        }
        for item in items {
            if self.items.len() == MAX_ITEMS {
                break;
            }
            if let Some(item) = item.bounded()
                && self.ids.insert(item.id.clone())
            {
                self.items.push(item);
            }
        }
        true
    }

    pub fn finish(&mut self, token: ModalToken) -> bool {
        if token != self.token || self.completed || !self.loading {
            return false;
        }
        self.loading = false;
        true
    }

    pub fn set_query(&mut self, query: &str, options: SearchOptions) -> Option<ModalQuery> {
        if self.completed {
            return None;
        }
        let query: String = query.trim().chars().take(4096).collect();
        if self.query == query && self.search == options {
            return None;
        }
        self.query = query;
        self.search = options;
        self.search_error = None;
        self.matcher = None;
        if self.options.dynamic {
            self.token.revision += 1;
            self.items.clear();
            self.ids.clear();
            self.loading = true;
            self.querying = true;
            return Some(ModalQuery {
                token: self.token,
                query: self.query.clone(),
                options,
            });
        }
        if self.query.is_empty() {
            return None;
        }
        let mut pattern = if options.regex {
            self.query.clone()
        } else {
            regex::escape(&self.query)
        };
        if options.whole_word {
            pattern = format!(r"\b(?:{pattern})\b");
        }
        match RegexBuilder::new(&pattern)
            .case_insensitive(!options.case_sensitive)
            .size_limit(1024 * 1024)
            .build()
        {
            Ok(matcher) => self.matcher = Some(matcher),
            Err(error) => self.search_error = Some(error.to_string()),
        }
        None
    }

    pub fn visible_items(&self) -> impl Iterator<Item = &ModalItem> {
        self.items.iter().filter(|item| self.matches(item))
    }

    fn matches(&self, item: &ModalItem) -> bool {
        if self.search_error.is_some() {
            return false;
        }
        if self.querying {
            return true;
        }
        self.matcher.as_ref().is_none_or(|matcher| {
            matcher.is_match(&item.title)
                || item
                    .subtitle
                    .as_ref()
                    .is_some_and(|value| matcher.is_match(value))
        })
    }

    pub fn complete(&mut self, selected: Option<&str>) -> Option<Option<ModalItem>> {
        if self.completed {
            return None;
        }
        let choice = match selected {
            Some(id) => Some(
                self.items
                    .iter()
                    .find(|item| item.id == id && self.matches(item))?
                    .clone(),
            ),
            None => None,
        };
        self.completed = true;
        self.loading = false;
        Some(choice)
    }
}

fn bounded(value: &str) -> String {
    value.chars().take(MAX_FIELD_CHARACTERS).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_stream_filters_subtitles_and_resolves_once() {
        let mut state = ModalState::new(ModalOptions::default(), Vec::new(), true);
        state.set_query(" RED ", SearchOptions::default());
        assert!(state.feed(
            state.token(),
            [
                ModalItem {
                    id: "apple".into(),
                    title: "Apple".into(),
                    subtitle: Some("red fruit".into())
                },
                ModalItem::new("pear", "Pear")
            ]
        ));
        assert_eq!(state.visible_items().count(), 1);
        assert_eq!(state.complete(Some("pear")), None);
        assert!(state.finish(state.token()));
        assert!(
            matches!(state.complete(Some("apple")), Some(Some(item)) if item.subtitle.as_deref() == Some("red fruit"))
        );
        assert_eq!(state.complete(None), None);
        assert!(!state.feed(state.token(), [ModalItem::new("late", "Late")]));
    }

    #[test]
    fn query_revisions_reject_late_batches_and_finishes() {
        let mut state = ModalState::new(
            ModalOptions {
                dynamic: true,
                ..ModalOptions::default()
            },
            vec![ModalItem::new("initial", "Initial")],
            true,
        );
        let initial = state.token();
        let first = state
            .set_query("one", SearchOptions::default())
            .map(|query| query.token);
        assert!(first.is_some());
        let next = state.set_query(
            " two ",
            SearchOptions {
                regex: true,
                ..SearchOptions::default()
            },
        );
        assert!(matches!(next, Some(query) if query.query == "two" && query.options.regex));
        assert!(!state.feed(initial, [ModalItem::new("stale", "Stale")]));
        assert!(!state.finish(first.unwrap_or(initial)));
        assert!(state.feed(state.token(), [ModalItem::new("latest", "Remote result")]));
        assert_eq!(state.visible_items().count(), 1);
        assert!(state.loading());
        assert!(state.finish(state.token()));
        assert_eq!(state.complete(None), Some(None));
    }

    #[test]
    fn row_caps_validation_and_search_options() {
        let mut state = ModalState::new(ModalOptions::default(), Vec::new(), true);
        state.feed(
            state.token(),
            [
                ModalItem::new("", "invalid"),
                ModalItem::new("a", "cat"),
                ModalItem::new("a", "duplicate"),
                ModalItem::new("b", "Catfish"),
            ],
        );
        state.set_query(
            "cat",
            SearchOptions {
                whole_word: true,
                ..SearchOptions::default()
            },
        );
        assert_eq!(state.visible_items().count(), 1);
        state.set_query(
            "Cat",
            SearchOptions {
                case_sensitive: true,
                ..SearchOptions::default()
            },
        );
        assert_eq!(state.visible_items().count(), 1);
        state.set_query(
            "[",
            SearchOptions {
                regex: true,
                ..SearchOptions::default()
            },
        );
        assert!(state.search_error().is_some());
        state.set_query("", SearchOptions::default());
        state.feed(
            state.token(),
            (0..=MAX_ITEMS).map(|i| ModalItem::new(i.to_string(), "界".repeat(201))),
        );
        assert_eq!(state.visible_items().count(), MAX_ITEMS);
        assert!(
            state
                .visible_items()
                .all(|item| item.title.chars().count() <= 200)
        );
        let other = ModalState::new(ModalOptions::default(), Vec::new(), true);
        assert!(!state.finish(other.token()));
    }
}

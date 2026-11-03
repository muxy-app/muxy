use gpui::{ListAlignment, ListState, px};
use muxy_app_core::agent_sessions::{PROVIDERS, Session};
use muxy_protocol::AgentProvider;
use muxy_ui::scrollbar::ListScrollbar;

pub(crate) type Key = (AgentProvider, String);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Recency {
    Recent,
    Week,
    Older,
}

impl Recency {
    fn of(session: &Session) -> Self {
        match session.modified.elapsed().unwrap_or_default().as_secs() {
            0..86_400 => Self::Recent,
            86_400..604_800 => Self::Week,
            _ => Self::Older,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Entry {
    Section(Recency),
    Session { index: usize, key: Key },
}

pub(crate) struct SessionList {
    pub(crate) state: ListState,
    pub(crate) scrollbar: ListScrollbar,
    pub(crate) entries: Vec<Entry>,
    pub(crate) selected: Option<Key>,
    pub(crate) counts: [usize; PROVIDERS.len()],
    filter: Option<(String, Vec<AgentProvider>)>,
}

impl SessionList {
    pub(crate) fn remeasure(&self) {
        muxy_ui::scrollbar::remeasure_list(&self.state);
    }
}

impl Default for SessionList {
    fn default() -> Self {
        Self {
            state: ListState::new(0, ListAlignment::Top, px(300.0)),
            scrollbar: ListScrollbar::default(),
            entries: Vec::new(),
            selected: None,
            counts: [0; PROVIDERS.len()],
            filter: None,
        }
    }
}

impl SessionList {
    pub(crate) fn rebuild(
        &mut self,
        sessions: &[Session],
        providers: &[AgentProvider],
        query: &str,
    ) -> Vec<usize> {
        let filter = (query.to_owned(), providers.to_vec());
        let reset = self.filter.as_ref() != Some(&filter);
        let mut offset = self.state.logical_scroll_top();
        let anchor = self.entries.get(offset.item_ix).cloned();
        self.counts.fill(0);
        let mut filtered = Vec::new();
        let mut entries = Vec::new();
        let mut previous = None;
        for (index, session) in sessions.iter().enumerate() {
            let Some(provider) = PROVIDERS
                .iter()
                .position(|provider| *provider == session.provider)
            else {
                continue;
            };
            if !session.matches(query) {
                continue;
            }
            self.counts[provider] += 1;
            if !providers.contains(&session.provider) {
                continue;
            }
            let recency = Recency::of(session);
            if previous != Some(recency) {
                entries.push(Entry::Section(recency));
            }
            previous = Some(recency);
            entries.push(Entry::Session {
                index,
                key: (session.provider, session.id.clone()),
            });
            filtered.push(index);
        }
        let selected = if reset { None } else { self.selected.take() };
        self.selected = selected.filter(|key| entries.iter().any(|entry| matches!(entry, Entry::Session { key: candidate, .. } if candidate == key)))
            .or_else(|| filtered.first().map(|index| (sessions[*index].provider, sessions[*index].id.clone())));
        if reset || entries != self.entries {
            self.state.reset(entries.len());
            self.scrollbar.reset();
            if !reset && let Some(anchor) = anchor {
                let position = entries.iter().position(|entry| match (&anchor, entry) {
                    (Entry::Session { key: a, .. }, Entry::Session { key: b, .. }) => a == b,
                    _ => *entry == anchor,
                });
                if let Some(position) = position {
                    offset.item_ix = position;
                    self.state.scroll_to(offset);
                }
            }
        }
        self.entries = entries;
        self.filter = Some(filter);
        filtered
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.entries.iter().find_map(|entry| match entry {
            Entry::Session { index, key } if Some(key) == self.selected.as_ref() => Some(*index),
            _ => None,
        })
    }

    pub(crate) fn advance(&mut self, backwards: bool) {
        let Some(current) = self.entries.iter().position(|entry| {
            matches!(entry, Entry::Session { key, .. } if Some(key) == self.selected.as_ref())
        }) else {
            return;
        };
        let is_session = |entry: &Entry| matches!(entry, Entry::Session { .. });
        let next = if backwards {
            self.entries[..current].iter().rposition(is_session)
        } else {
            self.entries[current + 1..]
                .iter()
                .position(is_session)
                .map(|index| current + 1 + index)
        };
        if let Some(row) = next
            && let Entry::Session { key, .. } = &self.entries[row]
        {
            self.selected = Some(key.clone());
            self.state.scroll_to_reveal_item(row);
        }
    }
}

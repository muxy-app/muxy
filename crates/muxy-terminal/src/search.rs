use std::time::Duration;
use unicode_segmentation::UnicodeSegmentation;

pub const SEARCH_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchDirection {
    Previous,
    Next,
}

impl SearchDirection {
    fn encoded(self) -> &'static str {
        match self {
            Self::Previous => "previous",
            Self::Next => "next",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchAction {
    Start,
    Query(String),
    Navigate(SearchDirection),
    End,
}

impl SearchAction {
    pub fn encode(&self) -> String {
        match self {
            Self::Start => String::from("start_search"),
            Self::Query(needle) => format!("search:{needle}"),
            Self::Navigate(direction) => format!("navigate_search:{}", direction.encoded()),
            Self::End => String::from("end_search"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchDispatch {
    Immediate(SearchAction),
    Debounced {
        action: SearchAction,
        delay: Duration,
    },
}

pub fn dispatch_for_query(needle: impl Into<String>) -> SearchDispatch {
    let needle = needle.into();
    let action = SearchAction::Query(needle.clone());
    if needle.is_empty() || needle.graphemes(true).nth(2).is_some() {
        SearchDispatch::Immediate(action)
    } else {
        SearchDispatch::Debounced {
            action,
            delay: SEARCH_DEBOUNCE,
        }
    }
}

pub fn match_display(total: Option<usize>, selected: Option<usize>) -> String {
    match (total, selected) {
        (None, _) => String::new(),
        (Some(1), None) => String::from("1 match"),
        (Some(total), None) => format!("{total} matches"),
        (Some(total), Some(selected)) => format!("{selected} of {total}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_exactly_the_ghostty_binding_action_names() {
        assert_eq!(SearchAction::Start.encode(), "start_search");
        assert_eq!(SearchAction::End.encode(), "end_search");
        assert_eq!(
            SearchAction::Query("需要".to_owned()).encode(),
            "search:需要"
        );
        assert_eq!(
            SearchAction::Navigate(SearchDirection::Previous).encode(),
            "navigate_search:previous"
        );
        assert_eq!(
            SearchAction::Navigate(SearchDirection::Next).encode(),
            "navigate_search:next"
        );
    }

    #[test]
    fn short_queries_debounce_and_empty_or_long_queries_dispatch_immediately() {
        assert!(matches!(
            dispatch_for_query(""),
            SearchDispatch::Immediate(_)
        ));
        assert!(matches!(
            dispatch_for_query("ab"),
            SearchDispatch::Debounced {
                delay: SEARCH_DEBOUNCE,
                ..
            }
        ));
        assert!(matches!(
            dispatch_for_query("abc"),
            SearchDispatch::Immediate(_)
        ));
    }

    #[test]
    fn debounce_counts_grapheme_clusters_not_bytes_or_scalars() {
        assert!(matches!(
            dispatch_for_query("é"),
            SearchDispatch::Debounced { .. }
        ));
        assert!(matches!(
            dispatch_for_query("👨‍👩‍👧‍👦"),
            SearchDispatch::Debounced { .. }
        ));
        assert!(matches!(
            dispatch_for_query("🇯🇵🇰🇷🇨🇳"),
            SearchDispatch::Immediate(_)
        ));
    }

    #[test]
    fn match_label_covers_absent_singular_plural_and_selected_counts() {
        assert_eq!(match_display(None, Some(1)), "");
        assert_eq!(match_display(Some(1), None), "1 match");
        assert_eq!(match_display(Some(9), None), "9 matches");
        assert_eq!(match_display(Some(9), Some(3)), "3 of 9");
    }
}

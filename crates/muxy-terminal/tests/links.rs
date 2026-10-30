use muxy_terminal::{LinkSpan, MAX_LINK_SPANS, MAX_LINK_URI, Size, Terminal};

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn osc(uri: &str, text: &str) -> String {
    format!("\x1b]8;;{uri}\x1b\\{text}\x1b]8;;\x1b\\")
}

#[test]
fn links_track_uri_only_changes_clear_resize_and_history_reads() -> Result {
    let mut terminal = Terminal::new(Size { cols: 40, rows: 3 }, 1_000_000)?;
    terminal.feed(osc("https://example.com", "link").as_bytes());
    terminal.take_changed_rows()?;
    assert_eq!(
        terminal.screen_links()[0].spans,
        vec![LinkSpan {
            start: 0,
            end: 4,
            uri: "https://example.com".into()
        }]
    );
    terminal.feed(format!("\r{}", osc("https://other.test", "link")).as_bytes());
    assert_eq!(terminal.take_changed_rows()?.len(), 1);
    assert_eq!(
        terminal.screen_links()[0].spans[0].uri,
        "https://other.test"
    );
    terminal.history(0..10)?;
    assert!(terminal.take_changed_rows()?.is_empty());
    assert_eq!(
        terminal.screen_links()[0].spans[0].uri,
        "https://other.test"
    );
    terminal.resize(Size { cols: 2, rows: 3 })?;
    terminal.take_changed_rows()?;
    assert!(
        terminal
            .screen_links()
            .iter()
            .flat_map(|row| &row.spans)
            .all(|span| span.end <= 2)
    );
    terminal.feed(b"\x1b[2J\x1b[H");
    terminal.take_changed_rows()?;
    assert!(terminal.screen_links().is_empty());
    Ok(())
}

#[test]
fn history_links_follow_page_rows_without_consuming_live_changes() -> Result {
    let mut terminal = Terminal::new(Size { cols: 40, rows: 3 }, 1_000_000)?;
    for index in 0..12 {
        terminal
            .feed(format!("{}\r\n", osc(&format!("https://test/{index}"), "界link")).as_bytes());
    }
    terminal.take_changed_rows()?;
    let screen_links = terminal.screen_links();
    terminal.compress_idle()?;
    let muxy_terminal::TerminalHistory { rows, links, .. } = terminal.history_with_links(3..8)?;
    assert_eq!(rows.len(), 5);
    for (index, row) in links.iter().enumerate() {
        assert_eq!(usize::from(row.row), index);
        assert_eq!(
            row.spans,
            [LinkSpan {
                start: 0,
                end: 6,
                uri: format!("https://test/{}", index + 3),
            }]
        );
    }
    assert_eq!(links.len(), rows.len());
    assert!(terminal.take_changed_rows()?.is_empty());
    assert_eq!(terminal.screen_links(), screen_links);
    terminal.feed(format!("\x1b[H{}", osc("https://changed.test", "界link")).as_bytes());
    terminal.history_with_links(0..5)?;
    assert_eq!(
        terminal.read_screen_links(MAX_LINK_SPANS)?[0].spans[0].uri,
        "https://changed.test"
    );
    assert_eq!(terminal.take_changed_rows()?.len(), 1);
    assert_eq!(
        terminal.screen_links()[0].spans[0].uri,
        "https://changed.test"
    );
    Ok(())
}

#[test]
fn history_link_budget_is_shared_across_viewports_and_renewed_for_each_page() -> Result {
    let mut terminal = Terminal::new(Size { cols: 40, rows: 3 }, 1_000_000)?;
    for index in 0..MAX_LINK_SPANS + 10 {
        terminal.feed(format!("{}\r\n", osc(&format!("https://test/{index}"), "link")).as_bytes());
    }
    let links = terminal.history_with_links(0..MAX_LINK_SPANS + 5)?.links;
    assert_eq!(
        links.iter().map(|row| row.spans.len()).sum::<usize>(),
        MAX_LINK_SPANS
    );
    let links = terminal
        .history_with_links(MAX_LINK_SPANS..MAX_LINK_SPANS + 5)?
        .links;
    assert_eq!(links.len(), 5);
    assert_eq!(links[0].row, 0);
    assert_eq!(
        links[0].spans[0].uri,
        format!("https://test/{MAX_LINK_SPANS}")
    );
    Ok(())
}

#[test]
fn uri_and_screen_budgets_are_bounded_and_recover_after_eviction() -> Result {
    let mut terminal = Terminal::new(
        Size {
            cols: 2048,
            rows: 2,
        },
        1_000_000,
    )?;
    terminal.feed(
        osc(
            &format!("https://example.com/{}", "x".repeat(MAX_LINK_URI)),
            "ignored",
        )
        .as_bytes(),
    );
    terminal.take_changed_rows()?;
    assert!(terminal.screen_links().is_empty());
    terminal.feed(b"\x1b[2J\x1b[H");
    for i in 0..MAX_LINK_SPANS + 10 {
        terminal.feed(osc(&format!("https://test/{i}"), "x").as_bytes());
    }
    terminal.take_changed_rows()?;
    assert_eq!(
        terminal
            .screen_links()
            .iter()
            .map(|row| row.spans.len())
            .sum::<usize>(),
        MAX_LINK_SPANS
    );
    terminal.feed(b"\x1b[H\x1b[2K\x1b[2;1H");
    terminal.feed(osc("https://recovered.test", "yes").as_bytes());
    terminal.take_changed_rows()?;
    assert_eq!(terminal.screen_links().len(), 1);
    assert_eq!(terminal.screen_links()[0].row, 1);
    assert_eq!(
        terminal.screen_links()[0].spans[0].uri,
        "https://recovered.test"
    );
    Ok(())
}

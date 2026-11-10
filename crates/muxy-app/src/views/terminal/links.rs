use std::ops::Range;

use gpui::{Context, Task};
use muxy_app_core::opener::Target;
use muxy_protocol::{LinkSpan, Run};

use super::{
    pane::TerminalPane,
    selection::{self, Point},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Candidate {
    pub(crate) row: isize,
    pub(crate) columns: Range<u16>,
    pub(crate) text: String,
    /// Wider stretches to try when `text` names no file, nearest first: few
    /// programs quote a path that has spaces in it, and a long one runs onto
    /// the next row.
    pub(crate) wider: Vec<(Vec<Span>, String)>,
}

/// The columns of one row that a link covers.
pub(crate) type Span = (isize, Range<u16>);

impl Candidate {
    /// The first stretch `resolve` knows, and the cells it covers.
    pub(crate) fn resolve(
        &self,
        resolve: impl Fn(&str) -> Option<Target>,
    ) -> Option<(Target, Vec<Span>)> {
        resolve(&self.text)
            .map(|target| (target, vec![(self.row, self.columns.clone())]))
            .or_else(|| {
                self.wider
                    .iter()
                    .find_map(|(spans, text)| Some((resolve(text)?, spans.clone())))
            })
    }
}

#[derive(Default)]
pub(crate) struct Hover {
    pub(crate) candidate: Option<Candidate>,
    pub(crate) target: Option<Target>,
    /// The cells the target covers.
    pub(crate) spans: Vec<Span>,
    task: Option<Task<()>>,
}

pub(crate) fn detect(runs: &[Run], point: Point, links: &[LinkSpan]) -> Option<Candidate> {
    if let Some(span) = links
        .iter()
        .find(|span| (span.start..span.end).contains(&point.column))
    {
        return Some(Candidate {
            row: point.row,
            columns: span.start..span.end,
            text: span.uri.clone(),
            wider: Vec::new(),
        });
    }
    let mut text = String::new();
    let mut mapping = Vec::new();
    for (columns, value) in selection::cells(runs) {
        let start = text.len();
        text.push_str(value);
        if text.len() > 65536 {
            return None;
        }
        mapping.push((start..text.len(), columns));
    }
    let byte = mapping
        .iter()
        .find(|(_, columns)| columns.contains(&point.column))?
        .0
        .start;
    let mut start = 0;
    while start < text.len() {
        let character = text[start..].chars().next()?;
        if character.is_whitespace() {
            start += character.len_utf8();
            continue;
        }
        let quoted = matches!(character, '\'' | '"');
        let body = start + if quoted { character.len_utf8() } else { 0 };
        let end = text[body..]
            .char_indices()
            .find(|(_, c)| {
                if quoted {
                    *c == character
                } else {
                    c.is_whitespace()
                }
            })
            .map_or(text.len(), |(offset, _)| body + offset);
        let value = trim(&text[body..end]);
        let offset = body + text[body..end].len()
            - text[body..end]
                .trim_start_matches(['(', '[', '<', '"', '\''])
                .len();
        let link_start = ["https://", "http://", "file://"]
            .into_iter()
            .filter_map(|prefix| value.find(prefix))
            .min()
            .unwrap_or(0);
        let value = &value[link_start..];
        let begin = offset + link_start;
        let finish = begin + value.len();
        if begin <= byte
            && byte < finish
            && !value.is_empty()
            && value.len() <= muxy_protocol::MAX_LINK_URI
        {
            let first = mapping
                .iter()
                .find(|(bytes, _)| bytes.contains(&begin))?
                .1
                .start;
            let last = mapping
                .iter()
                .rev()
                .find(|(bytes, _)| bytes.start < finish)?
                .1
                .end;
            return Some(Candidate {
                row: point.row,
                columns: first..last,
                text: value.into(),
                wider: Vec::new(),
            });
        }
        start = end
            + if quoted && end < text.len() {
                character.len_utf8()
            } else {
                0
            };
    }
    None
}

/// How many rows above and below the pointed one a path may run onto, how
/// many words past the pointed one, and how many stretches are tried at all.
const WIDER_ROWS: isize = 3;
const WIDER_WORDS: usize = 10;
const WIDER_LIMIT: usize = 128;

/// The stretches around `point` that could be a path with spaces in it, or
/// one that runs over several rows. Rows are joined as they were broken: in
/// the middle of a word when the row above is full to its last column, at a
/// space otherwise. A program that wraps its own text indents what it broke
/// in a word, so each stretch is also tried with the rows joined directly. An
/// empty row ends the text a path can be in.
pub(crate) fn widen<'a>(
    row: impl Fn(isize) -> Option<&'a [Run]>,
    width: u16,
    point: Point,
) -> Vec<(Vec<Span>, String)> {
    let line = |index: isize| {
        let cells = selection::cells(row(index)?);
        let blank = |cell: &(Range<u16>, &str)| cell.1.trim().is_empty();
        let first = cells.iter().position(|cell| !blank(cell))?;
        let last = cells.iter().rposition(|cell| !blank(cell))?;
        let full = last + 1 == cells.len() && cells[last].0.end >= width;
        Some((index, cells[first..=last].to_vec(), full))
    };
    let Some(pointed) = line(point.row) else {
        return Vec::new();
    };
    let mut lines = std::collections::VecDeque::from([pointed]);
    for distance in 1..=WIDER_ROWS {
        match point.row.checked_sub(distance).and_then(line) {
            Some(above) => lines.push_front(above),
            None => break,
        }
    }
    for distance in 1..=WIDER_ROWS {
        match point.row.checked_add(distance).and_then(line) {
            Some(below) => lines.push_back(below),
            None => break,
        }
    }
    let stretches = |tight: bool| {
        let mut text = String::new();
        let mut mapping = Vec::new();
        let mut broken_in_word = false;
        for (index, cells, full) in &lines {
            let continues_word = tight || (broken_in_word && cells[0].0.start == 0);
            if !text.is_empty() && !continues_word {
                text.push(' ');
            }
            for (columns, value) in cells {
                let start = text.len();
                text.push_str(value);
                mapping.push((start..text.len(), *index, columns.clone()));
            }
            broken_in_word = *full;
        }
        if text.len() > 65536 {
            return Vec::new();
        }
        let Some(byte) = mapping
            .iter()
            .find(|(_, index, columns)| *index == point.row && columns.contains(&point.column))
            .map(|(bytes, ..)| bytes.start)
        else {
            return Vec::new();
        };
        wider(&text, byte)
            .into_iter()
            .map(|bytes| {
                let mut spans: Vec<Span> = Vec::new();
                for (cell, index, columns) in &mapping {
                    if cell.start >= bytes.end || cell.end <= bytes.start {
                        continue;
                    }
                    match spans.last_mut() {
                        Some((last, covered)) if last == index => covered.end = columns.end,
                        _ => spans.push((*index, columns.clone())),
                    }
                }
                (spans, text[bytes].to_owned())
            })
            .collect::<Vec<_>>()
    };
    let spaced = stretches(false);
    let tight = if lines.len() > 1 {
        stretches(true)
    } else {
        Vec::new()
    };
    let mut merged: Vec<(Vec<Span>, String)> = Vec::new();
    let (mut spaced, mut tight) = (spaced.into_iter(), tight.into_iter());
    loop {
        let (first, second) = (spaced.next(), tight.next());
        if first.is_none() && second.is_none() {
            break;
        }
        for stretch in [first, second].into_iter().flatten() {
            if merged.len() < WIDER_LIMIT && !merged.contains(&stretch) {
                merged.push(stretch);
            }
        }
    }
    merged
}

/// The stretches of `text` around the word at `byte`: each takes in more
/// words before it, then after it.
fn wider(text: &str, byte: usize) -> Vec<Range<usize>> {
    let mut words: Vec<Range<usize>> = Vec::new();
    let mut open = None;
    for (index, character) in text.char_indices() {
        match (character.is_whitespace(), open) {
            (true, Some(start)) => {
                words.push(start..index);
                open = None;
            }
            (false, None) => open = Some(index),
            _ => {}
        }
    }
    if let Some(start) = open {
        words.push(start..text.len());
    }
    let Some(pointed) = words.iter().position(|word| word.contains(&byte)) else {
        return Vec::new();
    };
    let after = (words.len() - 1 - pointed).min(WIDER_WORDS);
    let mut stretches = Vec::new();
    for before in 0..=pointed {
        for after in 0..=after {
            if stretches.len() == WIDER_LIMIT {
                return stretches;
            }
            let raw = &text[words[pointed - before].start..words[pointed + after].end];
            let value = trim(raw);
            let begin = words[pointed - before].start + raw.len()
                - raw.trim_start_matches(['(', '[', '<', '"', '\'']).len();
            if !value.is_empty()
                && value.len() <= muxy_protocol::MAX_LINK_URI
                && !value.contains("://")
            {
                stretches.push(begin..begin + value.len());
            }
        }
    }
    stretches
}

fn trim(text: &str) -> &str {
    let mut text = text
        .trim_start_matches(['(', '[', '<', '"', '\''])
        .trim_end_matches(['.', ',', ';', '!', '"', '\'', '>']);
    loop {
        let pair = match text.chars().last() {
            Some(')') => ('(', ')'),
            Some(']') => ('[', ']'),
            Some('}') => ('{', '}'),
            _ => break,
        };
        if text.matches(pair.1).count() <= text.matches(pair.0).count() {
            break;
        }
        text = &text[..text.len() - 1];
    }
    text
}

impl TerminalPane {
    pub(crate) fn link_at(&self, position: gpui::Point<gpui::Pixels>) -> Option<Candidate> {
        if !self.native_visible || !self.geometry?.0.contains(&position) {
            return None;
        }
        let point = self.point_at(position, false)?;
        let grid = self.displayed_grid()?;
        let row = grid.history.len().checked_add_signed(point.row)?;
        let links = grid.content_links(row);
        let mut candidate = detect(grid.content_row(row)?, point, links)?;
        let explicit = links
            .iter()
            .any(|span| (span.start..span.end).contains(&point.column));
        if !explicit && !candidate.text.contains("://") {
            let history = grid.history.len();
            candidate.wider = widen(
                |row| grid.content_row(history.checked_add_signed(row)?),
                grid.size.cols,
                point,
            );
        }
        Some(candidate)
    }

    pub(crate) fn hover_link(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        command: bool,
        cx: &mut Context<Self>,
    ) {
        let candidate = command.then(|| self.link_at(position)).flatten();
        if self.link_hover.candidate == candidate {
            return;
        }
        self.link_hover = Hover {
            candidate: candidate.clone(),
            ..Hover::default()
        };
        cx.notify();
        let Some(candidate) = candidate else { return };
        if let Some(target) = Target::web_url(&candidate.text) {
            self.link_hover.target = Some(target);
            self.link_hover.spans = vec![(candidate.row, candidate.columns)];
            return;
        }
        let Some(context) = self.link_context() else {
            return;
        };
        if !context.server.is_local() {
            cx.emit(super::pane::PaneEvent::ResolveLink(candidate));
            return;
        }
        let wanted = candidate.clone();
        let Ok(result) = crate::opener::submit(move || {
            wanted.resolve(|text| crate::opener::resolve(text, &context))
        }) else {
            return;
        };
        self.link_hover.task = Some(cx.spawn(async move |pane, cx| {
            if let Ok(found) = result.recv().await {
                let _ = pane.update(cx, |pane, cx| pane.link_resolved(&candidate, found, cx));
            }
        }));
    }

    /// Another computer's server looked up a link; it counts if the pointer
    /// is still on it.
    pub(crate) fn link_resolved(
        &mut self,
        candidate: &Candidate,
        found: Option<(Target, Vec<Span>)>,
        cx: &mut Context<Self>,
    ) {
        if self.link_hover.candidate.as_ref() == Some(candidate) {
            let (target, spans) = found.unzip();
            self.link_hover.target = target;
            self.link_hover.spans = spans.unwrap_or_default();
            cx.notify();
        }
    }

    pub(crate) fn link_context(&self) -> Option<muxy_app_core::opener::OpenContext> {
        let mut context = self.open_context.clone()?;
        if let Some(directory) = self.directory() {
            context.directory = directory;
        }
        Some(context)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn run(text: &str, width: u16) -> Run {
        Run {
            text: text.into(),
            width,
            style: muxy_protocol::Style::default(),
        }
    }

    #[test]
    fn detection_trims_punctuation_but_keeps_balanced_urls_and_file_locations() {
        for (text, expected) in [
            ("https://example.com/a?b=1.", "https://example.com/a?b=1"),
            ("(https://example.com/a_(b)).", "https://example.com/a_(b)"),
            ("'https://example.com',", "https://example.com"),
            ("url=https://example.com/a,", "https://example.com/a"),
            ("/etc/hosts", "/etc/hosts"),
            ("src/main.rs:12:3", "src/main.rs:12:3"),
            ("\"/tmp/my file.rs\"", "/tmp/my file.rs"),
            ("file:///tmp/my%20file.rs", "file:///tmp/my%20file.rs"),
        ] {
            let byte = text.find(expected).unwrap();
            let candidate = detect(
                &[run(text, u16::try_from(text.len()).unwrap())],
                Point {
                    row: -1,
                    column: u16::try_from(byte + 2).unwrap(),
                },
                &[],
            )
            .unwrap();
            assert_eq!(candidate.text, expected, "{text}");
            assert_eq!(
                candidate.columns,
                u16::try_from(byte).unwrap()..u16::try_from(byte + expected.len()).unwrap()
            );
            assert_eq!(candidate.row, -1);
        }
    }

    fn found(rows: &[&str], width: u16, point: Point, file: &str) -> Option<(u32, Vec<Span>)> {
        let rows: Vec<_> = rows
            .iter()
            .map(|text| [run(text, u16::try_from(text.len()).unwrap())])
            .collect();
        let row = |index: isize| Some(rows.get(usize::try_from(index).ok()?)?.as_slice());
        let mut candidate = detect(row(point.row)?, point, &[])?;
        candidate.wider = widen(row, width, point);
        let (target, spans) = candidate.resolve(|text| {
            Target::file(text, std::path::Path::new("/"), None, |path| {
                path == std::path::Path::new(file)
            })
        })?;
        let Target::File(location) = target else {
            return None;
        };
        Some((location.line.unwrap_or(0), spans))
    }

    #[test]
    fn a_path_with_spaces_is_found_by_taking_in_the_words_around_it() {
        let text = "see /tmp/my dir/a b.rs:3, then";
        for column in [5, 12, 16, 19] {
            assert_eq!(
                found(&[text], 80, Point { row: 0, column }, "/tmp/my dir/a b.rs"),
                Some((3, vec![(0, 4..24)])),
                "column {column}"
            );
        }
    }

    #[test]
    fn a_path_is_followed_onto_the_rows_it_runs_over() {
        let wrapped = ["", "  /tmp/my dir/a", "  b c.rs:3 end", "", "c.rs"];
        for (row, column) in [(1, 4), (1, 10), (2, 2), (2, 5)] {
            assert_eq!(
                found(&wrapped, 80, Point { row, column }, "/tmp/my dir/a b c.rs"),
                Some((3, vec![(1, 2..15), (2, 2..10)])),
                "a break at a space, row {row} column {column}"
            );
        }
        let cut = ["see /tmp/l", "ong name.r", "s here"];
        for (row, column) in [(0, 6), (1, 1), (1, 6), (2, 0)] {
            assert_eq!(
                found(&cut, 10, Point { row, column }, "/tmp/long name.rs"),
                Some((0, vec![(0, 4..10), (1, 0..10), (2, 0..1)])),
                "a break inside a word, row {row} column {column}"
            );
        }
        let indented = ["  File: /tmp/dir/Presentation", "  s/over view.html"];
        for (row, column) in [(0, 10), (1, 3), (1, 9)] {
            assert_eq!(
                found(
                    &indented,
                    29,
                    Point { row, column },
                    "/tmp/dir/Presentations/over view.html"
                ),
                Some((0, vec![(0, 8..29), (1, 2..18)])),
                "a break inside a word of indented text, row {row} column {column}"
            );
        }
        assert_eq!(
            found(
                &wrapped,
                80,
                Point { row: 4, column: 1 },
                "/tmp/my dir/a b c.rs"
            ),
            None,
            "an empty row ends it"
        );
    }

    #[test]
    fn detection_respects_server_cell_widths_and_prefers_explicit_links() {
        let runs = [
            run("界", 2),
            run("e\u{301}", 1),
            run(" https://example.com", 20),
        ];
        let point = Point { row: 0, column: 5 };
        assert_eq!(detect(&runs, point, &[]).unwrap().columns, 4..23);
        let links = [LinkSpan {
            start: 0,
            end: 10,
            uri: "https://osc.test".into(),
        }];
        assert_eq!(
            detect(&runs, point, &links).unwrap().text,
            "https://osc.test"
        );
        assert!(detect(&runs, Point { row: 0, column: 3 }, &[]).is_none());
    }
}

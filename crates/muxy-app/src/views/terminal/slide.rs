use std::collections::VecDeque;
use std::time::{Duration, Instant};

use muxy_protocol::Run;

const DURATION: Duration = Duration::from_millis(120);
const MAX_LAG: f32 = 4.0;
const MAX_GHOSTS: usize = 5;
const TOLERANCE: usize = 2;
const MIN_ROWS: usize = 4;

#[derive(Debug, Default)]
pub(crate) struct Slide {
    from: f32,
    started: Option<Instant>,
    ghosts: VecDeque<Vec<Run>>,
    sampled: f32,
    history_total: Option<u64>,
    history_moved: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Growth {
    Rows(usize),
    Unknown,
}

impl Slide {
    pub(crate) fn clear(&mut self) {
        self.from = 0.0;
        self.started = None;
        self.sampled = 0.0;
        self.ghosts.clear();
    }

    pub(crate) fn history_changed(&mut self) {
        self.history_moved = true;
    }

    pub(crate) fn take_growth(&mut self, total: u64) -> Option<Growth> {
        let moved = std::mem::take(&mut self.history_moved);
        let previous = self.history_total.replace(total);
        if !moved {
            return None;
        }
        match previous {
            Some(previous) if total > previous => {
                usize::try_from(total - previous).ok().map(Growth::Rows)
            }
            Some(previous) if total < previous => None,
            _ => Some(Growth::Unknown),
        }
    }

    pub(crate) fn rows(&self, now: Instant) -> f32 {
        let Some(started) = self.started else {
            return 0.0;
        };
        let progress = (now.saturating_duration_since(started).as_secs_f32()
            / DURATION.as_secs_f32())
        .min(1.0);
        if progress >= 1.0 {
            return 0.0;
        }
        self.from * (1.0 - ease_out(progress))
    }

    pub(crate) fn sample(&mut self, now: Option<Instant>) -> f32 {
        self.sampled = now.map_or(0.0, |now| self.rows(now));
        self.sampled
    }

    pub(crate) fn sampled(&self) -> f32 {
        self.sampled
    }

    pub(crate) fn ghosts(&self) -> impl Iterator<Item = &[Run]> {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let count = self.sampled.ceil() as usize;
        self.ghosts.iter().take(count).map(Vec::as_slice)
    }

    pub(crate) fn observe(
        &mut self,
        before: &[Vec<Run>],
        after: &[Vec<Run>],
        growth: Growth,
        now: Instant,
    ) {
        let hint = match growth {
            Growth::Rows(rows) => Some(rows),
            Growth::Unknown => None,
        };
        match shift(before, after, hint) {
            Some(rows) => self.advance(rows, &before[..rows], now),
            None => self.clear(),
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn advance(&mut self, rows: usize, scrolled: &[Vec<Run>], now: Instant) {
        let remaining = self.rows(now);
        if remaining <= 0.0 {
            self.ghosts.clear();
        }
        self.from = (remaining + rows as f32).min(MAX_LAG);
        self.started = Some(now);
        for row in scrolled.iter().rev() {
            self.ghosts.push_front(row.clone());
        }
        self.ghosts.truncate(MAX_GHOSTS);
    }
}

fn blank(runs: &[Run]) -> bool {
    runs.iter().all(|run| run.text.trim().is_empty())
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

pub(crate) fn shift(before: &[Vec<Run>], after: &[Vec<Run>], hint: Option<usize>) -> Option<usize> {
    let height = before.len();
    if height != after.len() || height < MIN_ROWS {
        return None;
    }
    let limit = height / 2;
    let mismatches = |rows: usize| {
        let compared = height - rows;
        let mut count = 0;
        let mut solid = 0;
        for index in 0..compared {
            if after[index] != before[index + rows] {
                count += 1;
                if count > TOLERANCE {
                    return None;
                }
            } else if !blank(&after[index]) {
                solid += 1;
            }
        }
        (solid * 2 >= compared).then_some(count)
    };
    if let Some(rows) = hint.filter(|rows| (1..=limit).contains(rows))
        && mismatches(rows).is_some()
    {
        return Some(rows);
    }
    if hint.is_some_and(|rows| rows > limit) {
        return None;
    }
    (1..=limit)
        .filter_map(|rows| mismatches(rows).map(|count| (count, rows)))
        .min()
        .map(|(_, rows)| rows)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use muxy_protocol::Style;

    use super::*;

    fn line(text: &str) -> Vec<Run> {
        vec![Run {
            text: text.into(),
            width: u16::try_from(text.len()).unwrap(),
            style: Style::default(),
        }]
    }

    fn screen(lines: &[&str]) -> Vec<Vec<Run>> {
        lines.iter().map(|text| line(text)).collect()
    }

    #[test]
    fn finds_the_rows_new_output_pushed_up() {
        let before = screen(&["a", "b", "c", "d", "e", "f", "$ ls"]);
        let after = screen(&["c", "d", "e", "f", "$ ls", "x y", "$ "]);
        assert_eq!(shift(&before, &after, Some(2)), Some(2));
        assert_eq!(shift(&before, &after, None), Some(2));
    }

    #[test]
    fn tolerates_a_redrawn_prompt_line() {
        let before = screen(&["a", "b", "c", "d", "e", "f", "$ ls      12:00"]);
        let after = screen(&["b", "c", "d", "e", "f", "$ ls", "$ "]);
        assert_eq!(shift(&before, &after, Some(1)), Some(1));
    }

    #[test]
    fn redraws_and_large_bursts_do_not_slide() {
        let before = screen(&["a", "b", "c", "d", "e", "f", "g", "h"]);
        let redrawn = screen(&["1", "2", "3", "4", "5", "6", "7", "8"]);
        assert_eq!(shift(&before, &redrawn, Some(1)), None);
        assert_eq!(shift(&before, &redrawn, None), None);
        let burst = screen(&["f", "g", "h", "i", "j", "k", "l", "m"]);
        assert_eq!(shift(&before, &burst, Some(5)), None);
    }

    #[test]
    fn a_mostly_blank_screen_does_not_pass_for_a_scroll() {
        let before = screen(&["$ ls", "a", "b", "", "", "", "", ""]);
        let cleared = screen(&["$ ", "", "", "", "", "", "", ""]);
        assert_eq!(shift(&before, &cleared, Some(3)), None);
        assert_eq!(shift(&before, &cleared, None), None);
    }

    #[test]
    fn growth_needs_a_history_event_and_ignores_a_cleared_history() {
        let mut slide = Slide::default();
        assert_eq!(slide.take_growth(10), None);
        slide.history_changed();
        assert_eq!(slide.take_growth(13), Some(Growth::Rows(3)));
        assert_eq!(slide.take_growth(13), None);
        slide.history_changed();
        assert_eq!(slide.take_growth(13), Some(Growth::Unknown));
        slide.history_changed();
        assert_eq!(slide.take_growth(0), None);
    }

    #[test]
    fn consecutive_scrolls_accumulate_up_to_the_lag_and_keep_rows_in_order() {
        let start = Instant::now();
        let mut slide = Slide::default();
        let before = screen(&["a", "b", "c", "d", "e", "f"]);
        let after = screen(&["b", "c", "d", "e", "f", "g"]);
        slide.observe(&before, &after, Growth::Rows(1), start);
        assert_eq!(slide.rows(start), 1.0);
        let next = screen(&["c", "d", "e", "f", "g", "h"]);
        slide.observe(&after, &next, Growth::Rows(1), start);
        assert_eq!(slide.sample(Some(start)), 2.0);
        let ghosts: Vec<String> = slide.ghosts().map(|runs| runs[0].text.clone()).collect();
        assert_eq!(ghosts, ["b", "a"]);
        let later = screen(&["f", "g", "h", "i", "j", "k"]);
        slide.observe(&next, &later, Growth::Rows(3), start);
        assert_eq!(slide.rows(start), MAX_LAG);
        assert_eq!(slide.sample(Some(start + DURATION)), 0.0);
        assert_eq!(slide.ghosts().count(), 0);
    }

    #[test]
    fn an_unmatched_scroll_drops_stale_rows() {
        let start = Instant::now();
        let mut slide = Slide::default();
        let before = screen(&["a", "b", "c", "d", "e", "f"]);
        let after = screen(&["b", "c", "d", "e", "f", "g"]);
        slide.observe(&before, &after, Growth::Rows(1), start);
        let redrawn = screen(&["1", "2", "3", "4", "5", "6"]);
        slide.observe(&after, &redrawn, Growth::Rows(1), start);
        assert_eq!(slide.sample(Some(start)), 0.0);
        assert_eq!(slide.ghosts().count(), 0);
    }
}

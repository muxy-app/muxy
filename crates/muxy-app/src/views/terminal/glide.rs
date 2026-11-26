use std::time::{Duration, Instant};

const DURATION: Duration = Duration::from_millis(80);
const AFTER_INPUT: Duration = Duration::from_millis(500);

#[derive(Debug, Default)]
pub(crate) struct Glide {
    target: Option<(u16, u16)>,
    from: f32,
    started: Option<Instant>,
    typed: Option<Instant>,
    sampled: f32,
}

impl Glide {
    pub(crate) fn typed(&mut self, now: Instant) {
        self.typed = Some(now);
    }

    pub(crate) fn jump(&mut self) {
        self.target = None;
        self.started = None;
    }

    pub(crate) fn sample(
        &mut self,
        target: Option<(u16, u16)>,
        enabled: bool,
        now: Instant,
    ) -> f32 {
        let typing = self
            .typed
            .is_some_and(|typed| now.saturating_duration_since(typed) <= AFTER_INPUT);
        match (self.target, target) {
            (Some(previous), Some(next)) if previous == next => {}
            (Some((row, _)), Some((next_row, _))) if enabled && typing && row == next_row => {
                self.from = self.column(now);
                self.started = Some(now);
            }
            _ => self.started = None,
        }
        self.target = target;
        self.sampled = self.column(now);
        self.sampled
    }

    pub(crate) fn column_at(&self, row: u16, column: u16) -> f32 {
        if self.target == Some((row, column)) {
            self.sampled
        } else {
            f32::from(column)
        }
    }

    pub(crate) fn moving(&self, now: Instant) -> bool {
        self.started
            .is_some_and(|started| now.saturating_duration_since(started) < DURATION)
    }

    fn column(&self, now: Instant) -> f32 {
        let Some((_, column)) = self.target else {
            return 0.0;
        };
        let column = f32::from(column);
        let Some(started) = self.started else {
            return column;
        };
        let progress = (now.saturating_duration_since(started).as_secs_f32()
            / DURATION.as_secs_f32())
        .min(1.0);
        if progress >= 1.0 {
            return column;
        }
        self.from + (column - self.from) * ease_out(progress)
    }
}

fn ease_out(progress: f32) -> f32 {
    1.0 - (1.0 - progress).powi(3)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn typing_glides_along_the_line_and_lands_on_the_cell() {
        let start = Instant::now();
        let mut glide = Glide::default();
        assert_eq!(glide.sample(Some((3, 4)), true, start), 4.0);
        glide.typed(start);
        let moved = glide.sample(Some((3, 5)), true, start);
        assert_eq!(moved, 4.0);
        assert!(glide.moving(start));
        let halfway = glide.sample(Some((3, 5)), true, start + DURATION / 2);
        assert!(halfway > 4.0 && halfway < 5.0);
        assert_eq!(glide.sample(Some((3, 5)), true, start + DURATION), 5.0);
        assert!(!glide.moving(start + DURATION));
    }

    #[test]
    fn a_new_target_mid_glide_starts_from_where_the_cursor_is_shown() {
        let start = Instant::now();
        let mut glide = Glide::default();
        glide.sample(Some((0, 0)), true, start);
        glide.typed(start);
        glide.sample(Some((0, 10)), true, start);
        let shown = glide.sample(Some((0, 10)), true, start + DURATION / 2);
        assert_eq!(
            glide.sample(Some((0, 2)), true, start + DURATION / 2),
            shown
        );
    }

    #[test]
    fn program_moves_new_lines_and_a_disabled_setting_jump() {
        let start = Instant::now();
        let mut glide = Glide::default();
        glide.sample(Some((0, 10)), true, start);
        assert_eq!(glide.sample(Some((0, 0)), true, start), 0.0);
        glide.typed(start);
        assert_eq!(glide.sample(Some((1, 0)), true, start), 0.0);
        assert_eq!(glide.sample(Some((1, 8)), false, start), 8.0);
        let later = start + AFTER_INPUT + DURATION;
        assert_eq!(glide.sample(Some((1, 2)), true, later), 2.0);
        assert_eq!(glide.sample(None, true, later), 0.0);
        assert_eq!(glide.sample(Some((1, 6)), true, later), 6.0);
    }
}

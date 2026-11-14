//! The arithmetic of the tour's motion. Every scene is a pure function of the
//! time, so these are all it needs to move.

/// How far `t` has come through the stretch from `start`, from 0 to 1.
pub(super) fn progress(t: f32, start: f32, duration: f32) -> f32 {
    if duration <= 0.0 {
        return if t >= start { 1.0 } else { 0.0 };
    }
    ((t - start) / duration).clamp(0.0, 1.0)
}

/// Fast, then settling.
pub(super) fn ease_out(x: f32) -> f32 {
    1.0 - (1.0 - x).powi(3)
}

/// Slow at both ends.
pub(super) fn ease_in_out(x: f32) -> f32 {
    if x < 0.5 {
        4.0 * x * x * x
    } else {
        1.0 - (-2.0 * x + 2.0).powi(3) / 2.0
    }
}

/// Eased arrival from 0 to 1 over the stretch from `start`.
pub(super) fn rise(t: f32, start: f32, duration: f32) -> f32 {
    ease_out(progress(t, start, duration))
}

/// Eased from 0 to 1 and back: in at `start`, out by `end`, taking `fade` each way.
pub(super) fn span(t: f32, start: f32, end: f32, fade: f32) -> f32 {
    ease_in_out(progress(t, start, fade)) * (1.0 - ease_in_out(progress(t, end - fade, fade)))
}

/// A smooth swing between 0 and 1 every `period`, starting at 0.
pub(super) fn wave(t: f32, period: f32) -> f32 {
    0.5 - 0.5 * (t / period * std::f32::consts::TAU).cos()
}

/// Where `t` falls in a loop of `period`.
pub(super) fn cycle(t: f32, period: f32) -> f32 {
    t.rem_euclid(period)
}

pub(super) fn lerp(from: f32, to: f32, x: f32) -> f32 {
    from + (to - from) * x
}

/// `text` as typed so far, `rate` characters a second from `start`.
pub(super) fn typed(text: &str, t: f32, start: f32, rate: f32) -> &str {
    let elapsed = ((t - start) * rate).max(0.0);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Non-negative, and only ever a short line's length"
    )]
    let count = elapsed as usize;
    match text.char_indices().nth(count) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn progress_stays_between_its_ends() {
        assert!(close(progress(-1.0, 0.0, 2.0), 0.0));
        assert!(close(progress(1.0, 0.0, 2.0), 0.5));
        assert!(close(progress(9.0, 0.0, 2.0), 1.0));
    }

    #[test]
    fn a_stretch_without_length_is_a_step() {
        assert!(close(progress(0.9, 1.0, 0.0), 0.0));
        assert!(close(progress(1.0, 1.0, 0.0), 1.0));
    }

    #[test]
    fn easings_start_at_zero_and_end_at_one() {
        for easing in [ease_out, ease_in_out] {
            assert!(close(easing(0.0), 0.0));
            assert!(close(easing(1.0), 1.0));
        }
        assert!(ease_out(0.5) > 0.5);
        assert!(close(ease_in_out(0.5), 0.5));
    }

    #[test]
    fn a_span_comes_and_goes() {
        assert!(close(span(0.0, 1.0, 4.0, 0.5), 0.0));
        assert!(close(span(2.0, 1.0, 4.0, 0.5), 1.0));
        assert!(close(span(5.0, 1.0, 4.0, 0.5), 0.0));
    }

    #[test]
    fn a_wave_starts_low_and_peaks_halfway() {
        assert!(close(wave(0.0, 2.0), 0.0));
        assert!(close(wave(1.0, 2.0), 1.0));
        assert!(close(wave(2.0, 2.0), 0.0));
    }

    #[test]
    fn a_cycle_wraps() {
        assert!(close(cycle(7.0, 5.0), 2.0));
        assert!(close(cycle(-1.0, 5.0), 4.0));
    }

    #[test]
    fn typing_reveals_whole_characters_and_stops_at_the_end() {
        assert_eq!(typed("héllo", 0.0, 1.0, 10.0), "");
        assert_eq!(typed("héllo", 1.2, 1.0, 10.0), "hé");
        assert_eq!(typed("héllo", 9.0, 1.0, 10.0), "héllo");
    }
}

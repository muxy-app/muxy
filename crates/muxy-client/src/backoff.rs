use std::time::Duration;

/// How long to wait before connecting to another computer again. Backing off
/// spares its sshd and keeps tools like fail2ban from blocking this computer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Backoff {
    /// Someone is waiting at a prompt: 0.5, 1, 2 and 5 s, then 10 s, which
    /// repeats.
    Interactive,
    /// Nobody is waiting: 2 s, doubling up to a minute, which then repeats.
    Background,
}

const INTERACTIVE: [Duration; 5] = [
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(5),
    Duration::from_secs(10),
];

impl Backoff {
    /// The wait before retry number `retry`, counting from 0.
    pub fn delay(self, retry: u32) -> Duration {
        match self {
            Self::Interactive => {
                let step = usize::try_from(retry).unwrap_or(usize::MAX);
                INTERACTIVE[step.min(INTERACTIVE.len() - 1)]
            }
            Self::Background => Duration::from_secs((2_u64 << retry.min(5)).min(60)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(backoff: Backoff) -> Vec<f64> {
        [0, 1, 2, 3, 4, 5, 6, 100, u32::MAX]
            .into_iter()
            .map(|retry| backoff.delay(retry).as_secs_f64())
            .collect()
    }

    #[test]
    fn interactive_waits_stay_short_and_repeat_the_last() {
        assert_eq!(
            seconds(Backoff::Interactive),
            [0.5, 1.0, 2.0, 5.0, 10.0, 10.0, 10.0, 10.0, 10.0]
        );
    }

    #[test]
    fn background_waits_double_up_to_a_minute() {
        assert_eq!(
            seconds(Backoff::Background),
            [2.0, 4.0, 8.0, 16.0, 32.0, 60.0, 60.0, 60.0, 60.0]
        );
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScrollbarMetrics {
    pub total: u64,
    pub offset: u64,
    pub visible: u64,
}

impl ScrollbarMetrics {
    pub fn new(total: u64, offset: u64, visible: u64) -> Self {
        let visible = visible.min(total);
        let offset = offset.min(total.saturating_sub(visible));
        Self {
            total,
            offset,
            visible,
        }
    }

    pub fn maximum_offset(self) -> u64 {
        self.total.saturating_sub(self.visible)
    }

    pub fn is_scrollable(self) -> bool {
        self.visible > 0 && self.total > self.visible
    }
}

pub fn row_offset_for_thumb_origin(
    metrics: ScrollbarMetrics,
    thumb_origin: f64,
    track_length: f64,
    thumb_length: f64,
) -> u64 {
    let maximum = metrics.maximum_offset();
    let travel = track_length - thumb_length;
    if maximum == 0 || !thumb_origin.is_finite() || !travel.is_finite() || travel <= 0.0 {
        return metrics.offset.min(maximum);
    }

    let fraction_from_top = (thumb_origin / travel).clamp(0.0, 1.0);
    let rows_from_top = (fraction_from_top * maximum as f64).round();
    float_to_u64(rows_from_top).min(maximum)
}

fn float_to_u64(value: f64) -> u64 {
    if value <= 0.0 {
        0
    } else if value >= u64::MAX as f64 {
        u64::MAX
    } else {
        value as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_clamp_without_overflow() {
        assert_eq!(
            ScrollbarMetrics::new(10, u64::MAX, u64::MAX),
            ScrollbarMetrics {
                total: 10,
                offset: 0,
                visible: 10,
            }
        );
        assert_eq!(
            ScrollbarMetrics::new(u64::MAX, u64::MAX, 1).offset,
            u64::MAX - 1
        );
    }

    #[test]
    fn drag_mapping_round_trips_track_extremes() {
        let metrics = ScrollbarMetrics::new(100, 0, 20);
        assert_eq!(row_offset_for_thumb_origin(metrics, 0.0, 100.0, 20.0), 0);
        assert_eq!(row_offset_for_thumb_origin(metrics, 80.0, 100.0, 20.0), 80);
        assert_eq!(row_offset_for_thumb_origin(metrics, 40.0, 100.0, 20.0), 40);
    }
}

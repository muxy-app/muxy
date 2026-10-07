use std::collections::BTreeSet;

use gpui::{Bounds, Pixels, point, px, size};

pub(super) fn uncovered(
    outer: Bounds<Pixels>,
    colored: impl Iterator<Item = Bounds<Pixels>>,
) -> Vec<Bounds<Pixels>> {
    let rectangles: Vec<_> = colored
        .map(|bounds| bounds.intersect(&outer))
        .filter(|bounds| bounds.size.width > px(0.0) && bounds.size.height > px(0.0))
        .collect();
    let mut events = vec![
        (outer.top(), usize::MAX, false),
        (outer.bottom(), usize::MAX, false),
    ];
    for (index, bounds) in rectangles.iter().enumerate() {
        events.push((bounds.top(), index, true));
        events.push((bounds.bottom(), index, false));
    }
    events.sort_by(|a, b| f32::from(a.0).total_cmp(&f32::from(b.0)));
    let mut result = Vec::new();
    let mut active = BTreeSet::new();
    let mut top = outer.top();
    for (bottom, index, starts) in events {
        if bottom > top {
            let mut intervals: Vec<&Bounds<Pixels>> =
                active.iter().map(|&index| &rectangles[index]).collect();
            intervals.sort_by(|a, b| f32::from(a.left()).total_cmp(&f32::from(b.left())));
            let mut left = outer.left();
            for bounds in intervals {
                if bounds.left() > left {
                    result.push(Bounds::new(
                        point(left, top),
                        size(bounds.left() - left, bottom - top),
                    ));
                }
                left = left.max(bounds.right());
            }
            if left < outer.right() {
                result.push(Bounds::new(
                    point(left, top),
                    size(outer.right() - left, bottom - top),
                ));
            }
        }
        if index != usize::MAX {
            if starts {
                active.insert(index);
            } else {
                active.remove(&index);
            }
        }
        top = bottom;
    }
    result
}

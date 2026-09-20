use gpui::{Bounds, Hsla, Pixels, point, size};

use super::layers::Paths;

pub(super) fn prepare(
    mask: u32,
    bounds: Bounds<Pixels>,
    pixel: Pixels,
    color: Hsla,
    paths: &mut Paths,
) {
    let radius = (bounds.size.width / 8.0).max(pixel / 2.0);
    let disjoint =
        radius * 2.0 < bounds.size.width / 2.0 && radius * 2.0 < bounds.size.height / 4.0;
    let mut dots = Vec::with_capacity(mask.count_ones() as usize);
    for (bit, col, row) in [
        (0, 0_u8, 0_u8),
        (1, 0, 1),
        (2, 0, 2),
        (3, 1, 0),
        (4, 1, 1),
        (5, 1, 2),
        (6, 0, 3),
        (7, 1, 3),
    ] {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let center = point(
            bounds.left() + bounds.size.width * (0.25 + 0.5 * f32::from(col)),
            bounds.top() + bounds.size.height * (0.125 + 0.25 * f32::from(row)),
        );
        let dot = Bounds::new(
            center - point(radius, radius),
            size(radius * 2.0, radius * 2.0),
        );
        dots.push((dot, color));
    }
    if dots.is_empty() {
        return;
    }
    if disjoint {
        paths.dots(Some(bounds), dots);
    } else {
        for dot in dots {
            paths.dots(None, vec![dot]);
        }
    }
}

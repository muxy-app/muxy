use gpui::{Bounds, Hsla, PathBuilder, Pixels, point, px};

use super::{box_drawing::STROKES, dots, layers::Paths};

pub(super) fn prepare(
    text: &str,
    bounds: Bounds<Pixels>,
    color: Hsla,
    scale: f32,
    quads: &mut Vec<(Bounds<Pixels>, Hsla)>,
    paths: &mut Paths,
) -> bool {
    let mut chars = text.chars();
    let Some(ch) = chars.next() else {
        return false;
    };
    if chars.next().is_some() {
        return false;
    }
    let snap = |v: Pixels| px((f32::from(v) * scale).round() / scale);
    let bounds = Bounds::from_corners(bounds.origin.map(snap), bounds.bottom_right().map(snap));
    let pixel = px(1.0 / scale);
    let cp = u32::from(ch);
    if (0x2800..=0x28ff).contains(&cp) {
        #[cfg(test)]
        if paths.legacy_dots {
            braille(cp, bounds, pixel, color, paths);
            return true;
        }
        dots::prepare(cp - 0x2800, bounds, pixel, color, paths);
        return true;
    }
    if (0xe0b0..=0xe0b3).contains(&cp) {
        powerline(cp, bounds, pixel, color, paths);
        return true;
    }
    if !(0x2500..=0x257f).contains(&cp) {
        return false;
    }
    if (0x256d..=0x2570).contains(&cp) {
        rounded(cp, bounds, pixel, color, paths);
        return true;
    }
    if (0x2571..=0x2573).contains(&cp) {
        let mut path = PathBuilder::stroke(pixel);
        if cp != 0x2572 {
            path.move_to(bounds.bottom_left());
            path.line_to(bounds.top_right());
        }
        if cp != 0x2571 {
            path.move_to(bounds.origin);
            path.line_to(bounds.bottom_right());
        }
        if let Ok(path) = path.build() {
            paths.push((path, color));
        }
        return true;
    }
    strokes(cp, bounds, scale, quads, color);

    true
}

#[cfg(test)]
fn braille(cp: u32, bounds: Bounds<Pixels>, pixel: Pixels, color: Hsla, paths: &mut Paths) {
    let mask = cp - 0x2800;
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
        let x = bounds.left() + bounds.size.width * (0.25 + 0.5 * f32::from(col));
        let y = bounds.top() + bounds.size.height * (0.125 + 0.25 * f32::from(row));
        let mut path = PathBuilder::fill();
        path.move_to(point(x - radius, y));
        path.arc_to(
            point(radius, radius),
            px(0.0),
            false,
            true,
            point(x + radius, y),
        );
        path.arc_to(
            point(radius, radius),
            px(0.0),
            false,
            true,
            point(x - radius, y),
        );
        path.close();
        if let Ok(path) = path.build() {
            dots.push((path, color));
        }
    }
    if disjoint {
        paths.extend_disjoint(bounds, dots);
    } else {
        for dot in dots {
            paths.push(dot);
        }
    }
}
fn powerline(cp: u32, bounds: Bounds<Pixels>, pixel: Pixels, color: Hsla, paths: &mut Paths) {
    let center = bounds.center();
    let right = cp < 0xe0b2;
    let (edge, tip) = if right {
        (bounds.left(), bounds.right())
    } else {
        (bounds.right(), bounds.left())
    };
    let mut path = if cp.is_multiple_of(2) {
        PathBuilder::fill()
    } else {
        PathBuilder::stroke(pixel)
    };
    path.move_to(point(edge, bounds.top()));
    path.line_to(point(tip, center.y));
    path.line_to(point(edge, bounds.bottom()));
    if cp.is_multiple_of(2) {
        path.close();
    }
    if let Ok(path) = path.build() {
        paths.push((path, color));
    }
}
fn rounded(cp: u32, bounds: Bounds<Pixels>, pixel: Pixels, color: Hsla, paths: &mut Paths) {
    let center = bounds.center();
    let right = cp == 0x256d || cp == 0x2570;
    let down = cp == 0x256d || cp == 0x256e;
    let radius = (bounds.size.width / 2.0).min(bounds.size.height / 2.0);
    let dx = if right { radius } else { -radius };
    let dy = if down { radius } else { -radius };
    let mut path = PathBuilder::stroke(pixel);
    path.move_to(point(
        if right { bounds.right() } else { bounds.left() },
        center.y,
    ));
    path.line_to(center + point(dx, px(0.0)));
    path.arc_to(
        point(radius, radius),
        px(0.0),
        false,
        right != down,
        center + point(px(0.0), dy),
    );
    path.line_to(point(
        center.x,
        if down { bounds.bottom() } else { bounds.top() },
    ));
    if let Ok(path) = path.build() {
        paths.push((path, color));
    }
}

fn strokes(
    cp: u32,
    bounds: Bounds<Pixels>,
    scale: f32,
    quads: &mut Vec<(Bounds<Pixels>, Hsla)>,
    color: Hsla,
) {
    let snap = |v: Pixels| px((f32::from(v) * scale).round() / scale);
    let center = bounds.center().map(snap);
    let pixel = px(1.0 / scale);
    let (strokes, dash) = STROKES[usize::try_from(cp - 0x2500).unwrap_or(0)];
    for (direction, weight) in strokes.into_iter().enumerate() {
        if weight == 0 {
            continue;
        }
        let horizontal = direction < 2;
        let thickness = if weight == 2 { pixel * 2.0 } else { pixel };
        let offsets: &[f32] = if weight == 3 { &[-1.0, 1.0] } else { &[0.0] };
        for offset in offsets {
            let middle = if horizontal { center.y } else { center.x };
            let cross = snap(middle + pixel * *offset - thickness / 2.0);
            let start = match direction {
                0 => bounds.left(),
                2 => bounds.top(),
                _ => {
                    if horizontal {
                        center.x
                    } else {
                        center.y
                    }
                }
            };
            let end = match direction {
                1 => bounds.right(),
                3 => bounds.bottom(),
                _ => (if horizontal { center.x } else { center.y }) + thickness,
            };
            let mut segment = |a, b| {
                let quad = if horizontal {
                    Bounds::from_corners(point(a, cross), point(b, cross + thickness))
                } else {
                    Bounds::from_corners(point(cross, a), point(cross + thickness, b))
                };
                let quad = quad.intersect(&bounds);
                if quad.size.width > px(0.0) && quad.size.height > px(0.0) {
                    quads.push((quad, color));
                }
            };
            if dash == 0 {
                segment(start, end);
                continue;
            }
            let length = if horizontal {
                bounds.size.width
            } else {
                bounds.size.height
            };
            let origin = if horizontal {
                bounds.left()
            } else {
                bounds.top()
            };
            for part in 0..dash {
                let a = snap(origin + length * (f32::from(part) / f32::from(dash)));
                let b = snap(origin + length * ((f32::from(part) + 0.65) / f32::from(dash)));
                if b > start && a < end {
                    segment(a.max(start), b.min(end));
                }
            }
        }
    }
}

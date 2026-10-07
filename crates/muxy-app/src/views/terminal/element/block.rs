use gpui::{Bounds, Hsla, Pixels, point, px};

pub(super) fn quads(
    text: &str,
    bounds: Bounds<Pixels>,
    color: Hsla,
    scale: f32,
) -> Option<impl Iterator<Item = (Bounds<Pixels>, Hsla)>> {
    let eighths: &[[u8; 4]] = match text {
        "▀" => &[[0, 0, 8, 4]],
        "▁" => &[[0, 7, 8, 8]],
        "▂" => &[[0, 6, 8, 8]],
        "▃" => &[[0, 5, 8, 8]],
        "▄" => &[[0, 4, 8, 8]],
        "▅" => &[[0, 3, 8, 8]],
        "▆" => &[[0, 2, 8, 8]],
        "▇" => &[[0, 1, 8, 8]],
        "█" => &[[0, 0, 8, 8]],
        "▉" => &[[0, 0, 7, 8]],
        "▊" => &[[0, 0, 6, 8]],
        "▋" => &[[0, 0, 5, 8]],
        "▌" => &[[0, 0, 4, 8]],
        "▍" => &[[0, 0, 3, 8]],
        "▎" => &[[0, 0, 2, 8]],
        "▏" => &[[0, 0, 1, 8]],
        "▐" => &[[4, 0, 8, 8]],
        "▔" => &[[0, 0, 8, 1]],
        "▕" => &[[7, 0, 8, 8]],
        "▖" => &[[0, 4, 4, 8]],
        "▗" => &[[4, 4, 8, 8]],
        "▘" => &[[0, 0, 4, 4]],
        "▙" => &[[0, 0, 4, 4], [0, 4, 8, 8]],
        "▚" => &[[0, 0, 4, 4], [4, 4, 8, 8]],
        "▛" => &[[0, 0, 8, 4], [0, 4, 4, 8]],
        "▜" => &[[0, 0, 8, 4], [4, 4, 8, 8]],
        "▝" => &[[4, 0, 8, 4]],
        "▞" => &[[4, 0, 8, 4], [0, 4, 4, 8]],
        "▟" => &[[4, 0, 8, 4], [0, 4, 8, 8]],
        _ => return None,
    };
    Some(
        eighths
            .iter()
            .filter_map(move |&[left, top, right, bottom]| {
                let x = |fraction| {
                    snap(
                        bounds.left() + bounds.size.width * f32::from(fraction) / 8.0,
                        scale,
                    )
                };
                let y = |fraction| {
                    snap(
                        bounds.top() + bounds.size.height * f32::from(fraction) / 8.0,
                        scale,
                    )
                };
                let quad = Bounds::from_corners(point(x(left), y(top)), point(x(right), y(bottom)));
                (quad.size.width > px(0.0) && quad.size.height > px(0.0)).then_some((quad, color))
            })
            .collect::<Vec<_>>()
            .into_iter(),
    )
}

fn snap(value: Pixels, scale: f32) -> Pixels {
    px((f32::from(value) * scale).round() / scale)
}

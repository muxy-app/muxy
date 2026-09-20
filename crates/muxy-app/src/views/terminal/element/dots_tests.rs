use super::*;
use crate::views::terminal::element::{dots, glyph};
use gpui::{point, px, size};

fn circles(paths: &Paths) -> Vec<(Bounds<Pixels>, Hsla)> {
    paths
        .groups
        .iter()
        .flat_map(|group| match &group.shapes {
            Shapes::Dots(dots) => dots.clone(),
            Shapes::Paths(_) => Vec::new(),
        })
        .collect()
}

#[test]
fn every_braille_mask_keeps_its_dot_positions_and_transparency() {
    let positions = [
        (2.0, 2.0),
        (2.0, 6.0),
        (2.0, 10.0),
        (6.0, 2.0),
        (6.0, 6.0),
        (6.0, 10.0),
        (2.0, 14.0),
        (6.0, 14.0),
    ];
    let mut color = gpui::red();
    color.a = 0.5;
    for mask in 0..256_u32 {
        let mut paths = Paths::default();
        dots::prepare(
            mask,
            Bounds::new(point(px(0.0), px(0.0)), size(px(8.0), px(16.0))),
            px(0.5),
            color,
            &mut paths,
        );
        let actual = circles(&paths);
        let expected: Vec<_> = positions
            .iter()
            .enumerate()
            .filter(|(bit, _)| mask & (1 << bit) != 0)
            .map(|(_, &(x, y))| {
                (
                    Bounds::new(point(px(x - 1.0), px(y - 1.0)), size(px(2.0), px(2.0))),
                    color,
                )
            })
            .collect();
        assert_eq!(actual, expected, "mask {mask}");
        assert_eq!(paths.groups.len(), usize::from(mask != 0));
    }
}

#[test]
fn scaled_dots_use_snapped_cells_and_only_disjoint_circles_share_layers() {
    for scale in [1.0, 1.5, 2.0] {
        for (width, height) in [(7.75, 15.5), (8.0, 4.0), (0.5, 1.0)] {
            let mut paths = Paths::default();
            let mut quads = Vec::new();
            for column in 0..3_u8 {
                let x = 0.25 + f32::from(column) * width;
                let bounds = Bounds::new(point(px(x), px(0.25)), size(px(width), px(height)));
                assert!(glyph::prepare(
                    "⣿",
                    bounds,
                    gpui::blue(),
                    scale,
                    &mut quads,
                    &mut paths
                ));
                let snap = |v: f32| (v * scale).round() / scale;
                let snapped_width = snap(x + width) - snap(x);
                let radius = (snapped_width / 8.0).max(0.5 / scale);
                let actual = circles(&paths);
                let cell = &actual[usize::from(column) * 8..];
                assert_eq!(cell.len(), 8);
                for (dot, color) in cell {
                    assert_eq!(dot.size, size(px(radius * 2.0), px(radius * 2.0)));
                    assert_eq!(*color, gpui::blue());
                }
            }
            for group in &paths.groups {
                if let Shapes::Dots(dots) = &group.shapes {
                    for (index, &(bounds, _)) in dots.iter().enumerate() {
                        if let Some(group_bounds) = group.bounds {
                            assert_eq!(group_bounds.union(&bounds), group_bounds);
                        }
                        for &(other, _) in &dots[..index] {
                            assert!(!bounds.intersects(&other));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn circles_and_vector_shapes_keep_their_source_order() {
    let mut paths = Paths::default();
    let mut quads = Vec::new();
    for text in ["⣿", "╭", "⣿"] {
        assert!(glyph::prepare(
            text,
            Bounds::new(point(px(0.0), px(0.0)), size(px(8.0), px(4.0))),
            gpui::red(),
            1.0,
            &mut quads,
            &mut paths
        ));
    }
    assert_eq!(paths.groups.len(), 17);
    assert!(
        paths.groups[..8]
            .iter()
            .all(|g| matches!(g.shapes, Shapes::Dots(_)))
    );
    assert!(matches!(paths.groups[8].shapes, Shapes::Paths(_)));
    assert!(
        paths.groups[9..]
            .iter()
            .all(|g| matches!(g.shapes, Shapes::Dots(_)))
    );
    assert!(paths.groups.iter().all(|g| g.bounds.is_none()));
}

#[test]
fn adjacent_cells_batch_without_losing_per_cell_colors() {
    let mut paths = Paths::default();
    for column in 0..120_u16 {
        let mut color: Hsla = gpui::rgb(u32::from(column) * 100).into();
        color.a = 0.5;
        dots::prepare(
            255,
            Bounds::new(
                point(px(f32::from(column) * 8.0), px(0.0)),
                size(px(8.0), px(16.0)),
            ),
            px(0.5),
            color,
            &mut paths,
        );
    }
    assert_eq!(paths.groups.len(), 1);
    let dots = circles(&paths);
    assert_eq!(dots.len(), 960);
    for (column, cell) in dots.chunks_exact(8).enumerate() {
        let mut color: Hsla = gpui::rgb(u32::try_from(column).unwrap_or(0) * 100).into();
        color.a = 0.5;
        assert!(cell.iter().all(|(_, actual)| *actual == color));
    }
}

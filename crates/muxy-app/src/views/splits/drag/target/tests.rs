use super::*;
use gpui::size;
use muxy_app_core::Axis;

fn fixture() -> (Layout, [PaneId; 4], Bounds<Pixels>) {
    let [a, b, c, d] = std::array::from_fn(|_| PaneId::new());
    let mut layout = Layout::Leaf(a);
    layout.split(a, b, Direction::Right);
    layout.split(b, d, Direction::Down);
    layout.split(b, c, Direction::Right);
    (
        layout,
        [a, b, c, d],
        Bounds::new(point(px(271.0), px(33.0)), size(px(1001.0), px(701.0))),
    )
}

#[test]
fn shared_edges_offer_one_band_per_distinct_drop_with_the_widest_nearest_the_edge() {
    let (layout, [a, b, c, _], bounds) = fixture();
    assert_eq!(layout.dock_depths(c, Direction::Right), [3, 1]);
    assert_eq!(layout.dock_depths(b, Direction::Left), [3, 1]);
    assert_eq!(layout.dock_depths(a, Direction::Right), [1]);
    for scale in [1.0, 2.0] {
        let right = super::super::super::split_bounds(bounds, Axis::Horizontal, 0.5, scale)[2];
        for (position, pane, edge, level) in [
            (
                point(bounds.right() - px(6.0), bounds.top() + px(170.0)),
                c,
                Direction::Right,
                1,
            ),
            (
                point(bounds.right() - px(30.0), bounds.top() + px(170.0)),
                c,
                Direction::Right,
                0,
            ),
            (
                point(right.left() + px(6.0), bounds.top() + px(170.0)),
                b,
                Direction::Left,
                1,
            ),
            (
                point(right.left() + px(40.0), bounds.top() + px(170.0)),
                b,
                Direction::Left,
                0,
            ),
            (
                point(right.left() - px(6.0), bounds.top() + px(170.0)),
                a,
                Direction::Right,
                0,
            ),
        ] {
            assert_eq!(
                target_at(&layout, bounds, scale, position),
                Some(DropTarget::Dock { pane, edge, level })
            );
        }
    }
}

#[test]
fn both_sides_of_a_divider_and_the_gap_between_them_make_the_same_drop() {
    let (layout, [a, b, c, d], bounds) = fixture();
    for scale in [1.0, 2.0] {
        let [_, divider, right] =
            super::super::super::split_bounds(bounds, Axis::Horizontal, 0.5, scale);
        let top_row = bounds.top() + px(170.0);
        let results = [
            point(divider.center().x, top_row),
            point(divider.left() - px(6.0), top_row),
            point(right.left() + px(6.0), top_row),
        ]
        .map(|position| {
            target_at(&layout, bounds, scale, position)
                .unwrap()
                .apply(&layout, d)
                .unwrap()
        });
        assert_eq!(results[0], results[1]);
        assert_eq!(results[0], results[2]);
        assert_eq!(results[0].leaves(), [a, d, b, c]);
    }
}

#[test]
fn row_and_column_bands_win_where_a_divider_crosses_them() {
    let (layout, [a, _, c, _], bounds) = fixture();
    for scale in [1.0, 2.0] {
        let [_, divider, right] =
            super::super::super::split_bounds(bounds, Axis::Horizontal, 0.5, scale);
        let nested = super::super::super::split_bounds(right, Axis::Vertical, 0.5, scale)[1];
        for (position, pane, edge) in [
            (
                point(bounds.right() - px(6.0), nested.center().y),
                c,
                Direction::Right,
            ),
            (
                point(divider.center().x, bounds.top() + px(6.0)),
                a,
                Direction::Up,
            ),
        ] {
            assert_eq!(
                target_at(&layout, bounds, scale, position),
                Some(DropTarget::Dock {
                    pane,
                    edge,
                    level: 1
                })
            );
        }
    }
}

fn assert_levels_reachable(layout: &Layout, root: Bounds<Pixels>, scale: f32) {
    for (pane, bounds) in super::super::pane_rects(layout, root, scale) {
        for edge in EDGES {
            for level in 0..layout.dock_depths(pane, edge).len() {
                let expected = DropTarget::Dock { pane, edge, level };
                let reachable = (1_i16..80).any(|inset| {
                    [0.25, 0.5, 0.75].into_iter().any(|along| {
                        let inset = px(f32::from(inset));
                        let x = bounds.left() + bounds.size.width * along;
                        let y = bounds.top() + bounds.size.height * along;
                        let position = match edge {
                            Direction::Left => point(bounds.left() + inset, y),
                            Direction::Right => point(bounds.right() - inset, y),
                            Direction::Up => point(x, bounds.top() + inset),
                            Direction::Down => point(x, bounds.bottom() - inset),
                        };
                        target_at(layout, root, scale, position).as_ref() == Some(&expected)
                    })
                });
                assert!(reachable, "unreachable {expected:?} at {bounds:?}");
            }
        }
    }
}

#[test]
fn every_drop_level_of_every_pane_edge_remains_reachable_after_resizing() {
    let (mut layout, _, bounds) = fixture();
    for ratio in [0.3, 0.5, 0.8] {
        layout.set_ratio(&[], ratio).unwrap();
        for scale in [1.0, 2.0] {
            assert_levels_reachable(&layout, bounds, scale);
        }
    }
}

#[test]
fn corners_use_the_closest_physical_edge_instead_of_prioritizing_horizontal_edges() {
    let pane = PaneId::new();
    for dimensions in [size(px(1000.0), px(100.0)), size(px(100.0), px(1000.0))] {
        let bounds = Bounds::new(point(px(271.0), px(33.0)), dimensions);
        for (offset, edge) in [
            (point(px(20.0), px(2.0)), Direction::Up),
            (point(px(2.0), px(20.0)), Direction::Left),
            (
                point(dimensions.width - px(20.0), dimensions.height - px(2.0)),
                Direction::Down,
            ),
            (
                point(dimensions.width - px(2.0), dimensions.height - px(20.0)),
                Direction::Right,
            ),
        ] {
            assert_eq!(
                target_at(&Layout::Leaf(pane), bounds, 2.0, bounds.origin + offset),
                Some(DropTarget::Dock {
                    pane,
                    edge,
                    level: 0
                })
            );
        }
        assert_eq!(
            target_at(&Layout::Leaf(pane), bounds, 2.0, bounds.center()),
            Some(DropTarget::Swap(pane))
        );
    }
}

#[test]
fn divider_gaps_and_small_outer_overshoots_have_targets_but_outside_drops_do_not() {
    let (layout, _, bounds) = fixture();
    for scale in [1.0, 2.0] {
        let [_, divider, right] =
            super::super::super::split_bounds(bounds, Axis::Horizontal, 0.5, scale);
        let nested = super::super::super::split_bounds(right, Axis::Vertical, 0.5, scale)[1];
        for position in [
            divider.center(),
            nested.center(),
            point(bounds.right() + px(7.0), bounds.center().y),
        ] {
            assert!(target_at(&layout, bounds, scale, position).is_some());
        }
        assert!(
            target_at(
                &layout,
                bounds,
                scale,
                point(bounds.right() + px(9.0), bounds.center().y)
            )
            .is_none()
        );
    }
    assert!(target_at(&layout, Bounds::default(), 1.0, point(px(0.0), px(0.0))).is_none());
}

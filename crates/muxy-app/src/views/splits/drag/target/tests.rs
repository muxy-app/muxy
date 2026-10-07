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

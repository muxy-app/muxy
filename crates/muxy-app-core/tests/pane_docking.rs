#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use muxy_app_core::{AppState, Axis, Direction, Layout, PaneId};

const EDGES: [Direction; 4] = [
    Direction::Left,
    Direction::Right,
    Direction::Up,
    Direction::Down,
];

fn split(axis: Axis, ratio: f32, first: Layout, second: Layout) -> Layout {
    Layout::Split {
        axis,
        ratio,
        first: Box::new(first),
        second: Box::new(second),
    }
}

/// Each pane's `[x, y, width, height]` in a unit square, in reading order.
fn rects(layout: &Layout) -> Vec<(PaneId, [f32; 4])> {
    fn visit(layout: &Layout, rect: [f32; 4], result: &mut Vec<(PaneId, [f32; 4])>) {
        match layout {
            Layout::Leaf(pane) => result.push((*pane, rect)),
            Layout::Split {
                axis,
                ratio,
                first,
                second,
            } => {
                let index = usize::from(*axis == Axis::Vertical);
                let mut a = rect;
                let mut b = rect;
                a[index + 2] *= ratio;
                b[index] += a[index + 2];
                b[index + 2] -= a[index + 2];
                visit(first, a, result);
                visit(second, b, result);
            }
        }
    }
    let mut result = Vec::new();
    visit(layout, [0.0, 0.0, 1.0, 1.0], &mut result);
    result
}

fn assert_rects(layout: &Layout, expected: &[(PaneId, [f32; 4])]) {
    let actual = rects(layout);
    for (pane, rect) in expected {
        let (_, found) = actual
            .iter()
            .find(|(id, _)| id == pane)
            .unwrap_or_else(|| panic!("missing {pane:?} in {layout:?}"));
        assert!(
            found.iter().zip(rect).all(|(a, b)| (a - b).abs() < 1e-4),
            "{pane:?}: {found:?} != {rect:?}"
        );
    }
    assert_eq!(actual.len(), expected.len());
}

fn grid() -> (Layout, [PaneId; 4]) {
    let [a, b, c, d] = std::array::from_fn(|_| PaneId::new());
    (
        split(
            Axis::Horizontal,
            0.5,
            split(Axis::Vertical, 0.5, Layout::Leaf(a), Layout::Leaf(c)),
            split(Axis::Vertical, 0.5, Layout::Leaf(b), Layout::Leaf(d)),
        ),
        [a, b, c, d],
    )
}

const THIRD: f32 = 1.0 / 3.0;

#[test]
fn docking_at_the_workspace_edge_adds_an_equal_column_or_row() {
    for (axis, edge, perpendicular) in [
        (Axis::Horizontal, Direction::Right, Axis::Vertical),
        (Axis::Vertical, Direction::Down, Axis::Horizontal),
    ] {
        let [a, b, c] = std::array::from_fn(|_| PaneId::new());
        let layout = split(
            axis,
            0.5,
            Layout::Leaf(a),
            split(perpendicular, 0.5, Layout::Leaf(b), Layout::Leaf(c)),
        );
        assert_eq!(layout.dock_depths(b, edge), [2, 1]);
        let result = layout.docked(c, b, edge, 1).unwrap();
        result.validate().unwrap();
        assert_eq!(result.leaves(), [a, b, c]);
        let cell = |index: f32| match axis {
            Axis::Horizontal => [index * THIRD, 0.0, THIRD, 1.0],
            Axis::Vertical => [0.0, index * THIRD, 1.0, THIRD],
        };
        assert_rects(&result, &[(a, cell(0.0)), (b, cell(1.0)), (c, cell(2.0))]);
    }
}

#[test]
fn rows_and_columns_that_run_along_an_edge_add_no_extra_drop_levels() {
    let [a, b, c] = std::array::from_fn(|_| PaneId::new());
    let columns = split(
        Axis::Horizontal,
        0.5,
        Layout::Leaf(a),
        split(Axis::Horizontal, 0.5, Layout::Leaf(b), Layout::Leaf(c)),
    );
    assert_eq!(columns.dock_depths(c, Direction::Right), [1]);
    assert_eq!(columns.dock_depths(b, Direction::Left), [1]);
    assert_eq!(columns.dock_depths(b, Direction::Up), [1, 0]);
    let (grid, [a, b, ..]) = grid();
    assert_eq!(grid.dock_depths(b, Direction::Left), [2, 1]);
    assert_eq!(grid.dock_depths(b, Direction::Up), [2, 0]);
    assert_eq!(grid.dock_depths(a, Direction::Down), [2]);
    assert!(grid.dock_depths(PaneId::new(), Direction::Left).is_empty());
}

#[test]
fn moving_within_a_row_keeps_every_size_and_the_same_place_is_an_exact_noop() {
    let [a, b, c] = std::array::from_fn(|_| PaneId::new());
    let columns = split(
        Axis::Horizontal,
        0.5,
        Layout::Leaf(a),
        split(Axis::Horizontal, 0.5, Layout::Leaf(b), Layout::Leaf(c)),
    );
    assert_eq!(columns.moved(a, b, Some(Direction::Left)).unwrap(), columns);
    for (anchor, edge) in [(b, Direction::Right), (c, Direction::Left)] {
        let result = columns.moved(a, anchor, Some(edge)).unwrap();
        result.validate().unwrap();
        assert_rects(
            &result,
            &[
                (b, [0.0, 0.0, 0.25, 1.0]),
                (a, [0.25, 0.0, 0.5, 1.0]),
                (c, [0.75, 0.0, 0.25, 1.0]),
            ],
        );
    }
    for ratio in [0.15, 0.3, 0.61, 0.85] {
        for (axis, before, after) in [
            (Axis::Horizontal, Direction::Left, Direction::Right),
            (Axis::Vertical, Direction::Up, Direction::Down),
        ] {
            let layout = split(axis, ratio, Layout::Leaf(a), Layout::Leaf(b));
            assert_eq!(layout.docked(a, b, before, 0).unwrap(), layout);
            assert_eq!(layout.moved(b, a, Some(after)).unwrap(), layout);
            let swapped = layout.moved(a, b, Some(after)).unwrap();
            assert_eq!(
                swapped,
                split(
                    axis,
                    (1.0 - ratio).clamp(0.15, 0.85),
                    Layout::Leaf(b),
                    Layout::Leaf(a)
                )
            );
        }
    }
}

#[test]
fn joining_another_column_takes_an_equal_share_and_the_old_column_closes_up() {
    let (grid, [a, b, c, d]) = grid();
    for (anchor, edge) in [(b, Direction::Down), (d, Direction::Up)] {
        let result = grid.moved(a, anchor, Some(edge)).unwrap();
        result.validate().unwrap();
        assert_rects(
            &result,
            &[
                (c, [0.0, 0.0, 0.5, 1.0]),
                (b, [0.5, 0.0, 0.5, THIRD]),
                (a, [0.5, THIRD, 0.5, THIRD]),
                (d, [0.5, 2.0 * THIRD, 0.5, THIRD]),
            ],
        );
    }
    let beside = grid.moved(a, b, Some(Direction::Left)).unwrap();
    assert_rects(
        &beside,
        &[
            (c, [0.0, 0.0, 0.5, 1.0]),
            (a, [0.5, 0.0, 0.25, 0.5]),
            (b, [0.75, 0.0, 0.25, 0.5]),
            (d, [0.5, 0.5, 0.5, 0.5]),
        ],
    );
}

#[test]
fn a_column_left_with_one_pane_counts_as_that_pane() {
    let (grid, [a, b, c, d]) = grid();
    let expected = [
        (a, [0.0, 0.0, THIRD, 1.0]),
        (c, [THIRD, 0.0, THIRD, 1.0]),
        (b, [2.0 * THIRD, 0.0, THIRD, 0.5]),
        (d, [2.0 * THIRD, 0.5, THIRD, 0.5]),
    ];
    for level in 0..grid.dock_depths(c, Direction::Left).len() {
        assert_rects(
            &grid.docked(a, c, Direction::Left, level).unwrap(),
            &expected,
        );
    }
    assert_eq!(
        grid.docked(a, c, Direction::Right, 0).unwrap(),
        grid.docked(a, b, Direction::Left, 1).unwrap()
    );
}

#[test]
fn docking_beside_a_row_preserves_the_other_row_and_its_resized_splits() {
    let [source, top_left, top_right, bottom_left, bottom_right] =
        std::array::from_fn(|_| PaneId::new());
    let top = split(
        Axis::Horizontal,
        0.61,
        Layout::Leaf(top_left),
        Layout::Leaf(top_right),
    );
    let bottom = split(
        Axis::Horizontal,
        0.37,
        Layout::Leaf(bottom_left),
        Layout::Leaf(bottom_right),
    );
    let grid = split(Axis::Vertical, 0.7, top, bottom.clone());
    let layout = split(Axis::Horizontal, 0.3, Layout::Leaf(source), grid.clone());
    let beside_row = layout.docked(source, top_left, Direction::Left, 0).unwrap();
    let Layout::Split {
        axis: Axis::Vertical,
        ratio,
        second,
        ..
    } = &beside_row
    else {
        panic!("{beside_row:?}");
    };
    assert!((ratio - 0.7).abs() < f32::EPSILON);
    assert_eq!(**second, bottom);
    assert_rects(
        &beside_row,
        &[
            (source, [0.0, 0.0, THIRD, 0.7]),
            (top_left, [THIRD, 0.0, 0.61 * 2.0 * THIRD, 0.7]),
            (
                top_right,
                [THIRD + 0.61 * 2.0 * THIRD, 0.0, 0.39 * 2.0 * THIRD, 0.7],
            ),
            (bottom_left, [0.0, 0.7, 0.37, 0.3]),
            (bottom_right, [0.37, 0.7, 0.63, 0.3]),
        ],
    );
    assert_eq!(layout.dock_depths(top_left, Direction::Up), [3, 2, 0]);
    let above_grid = layout.docked(source, top_left, Direction::Up, 2).unwrap();
    let Layout::Split {
        axis: Axis::Vertical,
        ratio,
        first,
        second,
    } = &above_grid
    else {
        panic!("{above_grid:?}");
    };
    assert!((ratio - THIRD).abs() < 1e-6);
    assert_eq!(**first, Layout::Leaf(source));
    assert_eq!(**second, grid);
}

#[test]
fn every_drop_keeps_each_pane_once_in_a_valid_layout_and_never_targets_itself() {
    let [source, top_left, top_right, bottom_left, bottom_right] =
        std::array::from_fn(|_| PaneId::new());
    let mut layout = Layout::Leaf(source);
    layout.split(source, top_left, Direction::Right);
    layout.split(top_left, top_right, Direction::Down);
    layout.split(top_right, bottom_left, Direction::Right);
    layout.split(source, bottom_right, Direction::Up);
    layout.set_ratio(&[], 0.61).unwrap();
    let panes = [source, top_left, top_right, bottom_left, bottom_right];
    let mut original = layout.leaves();
    original.sort_unstable();
    for pane in panes {
        for anchor in panes {
            for edge in EDGES {
                let levels = layout.dock_depths(anchor, edge).len();
                assert!(levels >= 1);
                for level in 0..levels {
                    let result = layout.docked(pane, anchor, edge, level);
                    if pane == anchor && level == 0 {
                        assert!(result.is_none());
                        continue;
                    }
                    let result = result.unwrap();
                    result.validate().unwrap();
                    let mut leaves = result.leaves();
                    leaves.sort_unstable();
                    assert_eq!(leaves, original, "{pane:?} {anchor:?} {edge:?} {level}");
                }
                assert!(layout.docked(pane, anchor, edge, levels).is_none());
            }
        }
    }
    assert!(
        layout
            .docked(PaneId::new(), source, Direction::Left, 0)
            .is_none()
    );
    assert!(
        Layout::Leaf(source)
            .docked(source, source, Direction::Left, 0)
            .is_none()
    );
}

#[test]
fn state_docking_is_atomic_preserves_records_and_round_trips() {
    let mut state = AppState::bootstrap().unwrap();
    state.open_terminal_tab(state.home().id).unwrap();
    let a = state.window().active_pane.unwrap();
    let b = state.split_pane(a, Direction::Right).unwrap();
    let c = state.split_pane(b, Direction::Down).unwrap();
    let records = state.home().tabs[0].panes.clone();
    state.dock_pane(a, b, Direction::Up, 1).unwrap();
    assert_eq!(state.home().tabs[0].layout.leaves(), [a, b, c]);
    assert_eq!(state.window().active_pane, Some(a));
    assert_eq!(state.home().tabs[0].panes, records);
    let bytes = serde_json::to_vec(&state).unwrap();
    assert_eq!(serde_json::from_slice::<AppState>(&bytes).unwrap(), state);
    let before = state.clone();
    assert!(state.dock_pane(a, a, Direction::Left, 0).is_err());
    assert!(state.dock_pane(a, b, Direction::Left, 9).is_err());
    assert_eq!(state, before);
}

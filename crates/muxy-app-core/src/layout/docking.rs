//! Pane drops see a layout as rows and columns rather than binary splits: a run
//! of same-direction splits is one row or column, so a pane dropped beside
//! another pane in a row joins that row.

use super::{Axis, Direction, Layout};
use crate::PaneId;

impl Layout {
    /// The distinct drops along `edge` of `anchor`, innermost first: beside
    /// `anchor`, then beside each enclosing row or column that shares that
    /// edge. Each is given as how deeply its pane, row, or column is nested,
    /// with 0 for the whole layout, so drops on different edges compare.
    pub fn dock_depths(&self, anchor: PaneId, edge: Direction) -> Vec<usize> {
        Node::from(self)
            .levels(anchor, edge)
            .iter()
            .map(Vec::len)
            .collect()
    }

    /// Moves `pane` to the `edge` of `anchor`, or of the enclosing row or
    /// column that `level` picks from [`Layout::dock_depths`].
    ///
    /// A pane joining a row or column gets an equal share of it, a pane moved
    /// within its own row or column keeps its size, and the panes it leaves
    /// grow evenly into its space.
    pub fn docked(
        &self,
        pane: PaneId,
        anchor: PaneId,
        edge: Direction,
        level: usize,
    ) -> Option<Self> {
        let mut root = Node::from(self);
        let target = root.levels(anchor, edge).into_iter().nth(level)?;
        if root.dock(pane, &target, edge)? {
            root.into_layout()
        } else {
            Some(self.clone())
        }
    }
}

enum Node {
    Pane(PaneId),
    Group(Group),
}

/// A row or column: one run of same-direction splits.
struct Group {
    axis: Axis,
    members: Vec<Member>,
    /// The original splits, reused until the members change.
    shape: Option<Shape>,
}

struct Member {
    /// Share of the group, from 0 to 1.
    size: f32,
    node: Node,
}

enum Shape {
    Member,
    Split {
        ratio: f32,
        first: Box<Shape>,
        second: Box<Shape>,
    },
}

impl From<&Layout> for Node {
    fn from(layout: &Layout) -> Self {
        match layout {
            Layout::Leaf(pane) => Self::Pane(*pane),
            Layout::Split { axis, .. } => {
                let mut members = Vec::new();
                let shape = Shape::collect(layout, *axis, 1.0, &mut members);
                Self::Group(Group {
                    axis: *axis,
                    members,
                    shape: Some(shape),
                })
            }
        }
    }
}

impl Node {
    fn into_layout(self) -> Option<Layout> {
        match self {
            Self::Pane(pane) => Some(Layout::Leaf(pane)),
            Self::Group(Group {
                axis,
                members,
                shape: Some(shape),
            }) => shape.build(axis, &mut members.into_iter()),
            Self::Group(Group {
                axis,
                members,
                shape: None,
            }) => balanced(axis, members),
        }
    }

    /// Paths of the nodes a pane can dock against along `edge` of `anchor`,
    /// innermost first. Rows or columns that run along the edge only repeat
    /// the drop beside their end member, so they are skipped.
    fn levels(&self, anchor: PaneId, edge: Direction) -> Vec<Vec<usize>> {
        let Some(path) = self.find(anchor) else {
            return Vec::new();
        };
        let mut levels = vec![path.clone()];
        for depth in (0..path.len()).rev() {
            let Some(Self::Group(group)) = self.at(&path[..depth]) else {
                break;
            };
            if group.axis != edge.axis() {
                levels.push(path[..depth].to_vec());
            } else if Some(path[depth]) != group.end(edge) {
                break;
            }
        }
        levels
    }

    /// Moves `pane` beside the node at `target`, returning whether anything
    /// changed.
    fn dock(&mut self, pane: PaneId, target: &[usize], edge: Direction) -> Option<bool> {
        let axis = edge.axis();
        let after = usize::from(matches!(edge, Direction::Right | Direction::Down));
        if self.at(target)?.is_pane(pane) {
            return None;
        }
        let target = match target.split_last() {
            Some((_, parent)) if self.dissolves_without(parent, pane, axis) => parent,
            _ => target,
        };
        let (container, index) = match self.at(target)? {
            Self::Group(group) if group.axis == axis => {
                (target.to_vec(), group.members.len() * after)
            }
            _ => match target.split_last() {
                Some((index, parent)) if self.at(parent).and_then(Self::axis) == Some(axis) => {
                    (parent.to_vec(), index + after)
                }
                _ => {
                    self.at_mut(target)?.wrap(axis);
                    (target.to_vec(), after)
                }
            },
        };
        let mut source = self.find(pane)?;
        let share = 1.0 / (self.at(&container)?.lanes(axis, pane) + 1.0);
        let Self::Group(group) = self.at_mut(&container)? else {
            return None;
        };
        if source.len() == container.len() + 1 && source.starts_with(&container) {
            let from = source[container.len()];
            let to = if from < index { index - 1 } else { index };
            if from == to {
                return Some(false);
            }
            let member = group.members.remove(from);
            group.members.insert(to, member);
            group.shape = None;
            return Some(true);
        }
        for member in &mut group.members {
            member.size *= 1.0 - share;
        }
        group.members.insert(
            index,
            Member {
                size: share,
                node: Self::Pane(pane),
            },
        );
        group.shape = None;
        if source.starts_with(&container) && source[container.len()] >= index {
            source[container.len()] += 1;
        }
        self.remove(&source)?;
        Some(true)
    }

    /// Whether the group at `path` runs across `axis` and holds only `pane`
    /// and one other member, which takes the group's place once `pane` moves.
    /// Docking beside that member then means docking beside the group.
    fn dissolves_without(&self, path: &[usize], pane: PaneId, axis: Axis) -> bool {
        let Some(Self::Group(group)) = self.at(path) else {
            return false;
        };
        group.axis != axis
            && group.members.len() == 2
            && group.members.iter().any(|member| member.node.is_pane(pane))
    }

    fn is_pane(&self, pane: PaneId) -> bool {
        matches!(self, Self::Pane(id) if *id == pane)
    }

    fn axis(&self) -> Option<Axis> {
        match self {
            Self::Pane(_) => None,
            Self::Group(group) => Some(group.axis),
        }
    }

    /// Removes the member at `path`; the rest of its row or column grows
    /// evenly into its space, and a group left with one member dissolves.
    fn remove(&mut self, path: &[usize]) -> Option<()> {
        let (index, parent) = path.split_last()?;
        let node = self.at_mut(parent)?;
        let Self::Group(group) = node else {
            return None;
        };
        if *index >= group.members.len() {
            return None;
        }
        group.members.remove(*index);
        if group.members.len() == 1 {
            *node = group.members.pop()?.node;
        } else {
            let total: f32 = group.members.iter().map(|member| member.size).sum();
            for member in &mut group.members {
                member.size /= total;
            }
            group.shape = None;
        }
        Some(())
    }

    /// Turns this node into the only member of a new group along `axis`.
    fn wrap(&mut self, axis: Axis) {
        let node = std::mem::replace(
            self,
            Self::Group(Group {
                axis,
                members: Vec::new(),
                shape: None,
            }),
        );
        if let Self::Group(group) = self {
            group.members.push(Member { size: 1.0, node });
        }
    }

    /// Panes side by side along `axis`, as if `without` were already gone.
    fn lanes(&self, axis: Axis, without: PaneId) -> f32 {
        match self {
            Self::Pane(pane) => {
                if *pane == without {
                    0.0
                } else {
                    1.0
                }
            }
            Self::Group(group) => {
                let lanes = group
                    .members
                    .iter()
                    .map(|member| member.node.lanes(axis, without));
                if group.axis == axis {
                    lanes.sum()
                } else {
                    lanes.fold(0.0, f32::max)
                }
            }
        }
    }

    fn find(&self, pane: PaneId) -> Option<Vec<usize>> {
        match self {
            Self::Pane(id) => (*id == pane).then(Vec::new),
            Self::Group(group) => group
                .members
                .iter()
                .enumerate()
                .find_map(|(index, member)| {
                    let mut path = member.node.find(pane)?;
                    path.insert(0, index);
                    Some(path)
                }),
        }
    }

    fn at(&self, path: &[usize]) -> Option<&Self> {
        let Some((index, rest)) = path.split_first() else {
            return Some(self);
        };
        let Self::Group(group) = self else {
            return None;
        };
        group.members.get(*index)?.node.at(rest)
    }

    fn at_mut(&mut self, path: &[usize]) -> Option<&mut Self> {
        let Some((index, rest)) = path.split_first() else {
            return Some(self);
        };
        let Self::Group(group) = self else {
            return None;
        };
        group.members.get_mut(*index)?.node.at_mut(rest)
    }
}

impl Group {
    /// Index of the member that touches `edge`, which must run across the
    /// group.
    fn end(&self, edge: Direction) -> Option<usize> {
        match edge {
            Direction::Left | Direction::Up => Some(0),
            Direction::Right | Direction::Down => self.members.len().checked_sub(1),
        }
    }
}

impl Shape {
    fn collect(layout: &Layout, axis: Axis, size: f32, members: &mut Vec<Member>) -> Self {
        match layout {
            Layout::Split {
                axis: split,
                ratio,
                first,
                second,
            } if *split == axis => Self::Split {
                ratio: *ratio,
                first: Box::new(Self::collect(first, axis, size * ratio, members)),
                second: Box::new(Self::collect(second, axis, size * (1.0 - ratio), members)),
            },
            _ => {
                members.push(Member {
                    size,
                    node: Node::from(layout),
                });
                Self::Member
            }
        }
    }

    fn build(self, axis: Axis, members: &mut impl Iterator<Item = Member>) -> Option<Layout> {
        match self {
            Self::Member => members.next()?.node.into_layout(),
            Self::Split {
                ratio,
                first,
                second,
            } => Some(Layout::Split {
                axis,
                ratio,
                first: Box::new(first.build(axis, members)?),
                second: Box::new(second.build(axis, members)?),
            }),
        }
    }
}

/// Rebuilds a changed row or column as splits that each divide it as close to
/// the middle as its members allow.
fn balanced(axis: Axis, mut members: Vec<Member>) -> Option<Layout> {
    if members.len() < 2 {
        return members.pop()?.node.into_layout();
    }
    let total: f32 = members.iter().map(|member| member.size).sum();
    let mut before = 0.0;
    let (index, ratio) = members[..members.len() - 1]
        .iter()
        .map(|member| {
            before += member.size;
            before / total
        })
        .enumerate()
        .min_by(|(_, a), (_, b)| (a - 0.5).abs().total_cmp(&(b - 0.5).abs()))?;
    let second = members.split_off(index + 1);
    Some(Layout::Split {
        axis,
        ratio: ratio.clamp(0.15, 0.85),
        first: Box::new(balanced(axis, members)?),
        second: Box::new(balanced(axis, second)?),
    })
}

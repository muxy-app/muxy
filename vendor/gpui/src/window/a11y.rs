use crate::{App, Bounds, FocusId, Pixels, SharedString, Window};
use accesskit::{Action, NodeId, TreeUpdate};
use collections::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) const ROOT_NODE_ID: NodeId = NodeId(0);

pub(crate) type A11yActionListener =
    Box<dyn FnMut(Option<&accesskit::ActionData>, &mut Window, &mut App) + 'static>;

pub(crate) struct A11y {
    active_flag: Arc<AtomicBool>,
    active_this_frame: bool,
    pub(crate) nodes: A11yNodeBuilder,
    pub(crate) focus_ids: FxHashMap<NodeId, FocusId>,
    pub(crate) node_bounds: FxHashMap<NodeId, Bounds<Pixels>>,
    pub(crate) action_listeners: FxHashMap<NodeId, Vec<(Action, A11yActionListener)>>,
    window_title: Option<SharedString>,
}

impl A11y {
    pub(crate) fn new(active_flag: Arc<AtomicBool>, window_title: Option<SharedString>) -> Self {
        Self {
            active_flag,
            active_this_frame: false,
            nodes: A11yNodeBuilder::new(),
            focus_ids: FxHashMap::default(),
            node_bounds: FxHashMap::default(),
            action_listeners: FxHashMap::default(),
            window_title,
        }
    }

    pub(crate) fn set_window_title(&mut self, title: impl Into<SharedString>) {
        self.window_title = Some(title.into());
    }

    pub(crate) fn sync_active_flag(&mut self) {
        self.active_this_frame = self.active_flag.load(Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn activate_for_test(&mut self) {
        self.active_flag.store(true, Ordering::SeqCst);
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active_this_frame
    }

    pub(crate) fn begin_frame(&mut self) {
        self.focus_ids.clear();
        self.node_bounds.clear();
        self.action_listeners.clear();
        self.nodes.begin_frame(self.window_title.as_ref());
    }

    pub(crate) fn end_frame(&mut self) -> TreeUpdate {
        self.nodes.finalize()
    }
}

#[derive(Clone, Copy)]
pub(crate) struct A11yCheckpoint {
    nodes: usize,
    children: usize,
    focus: NodeId,
    #[cfg(debug_assertions)]
    has_set_focus: bool,
}

pub(crate) struct A11yNodeBuilder {
    ids_stack: SmallVec<[NodeId; 16]>,
    nodes_stack: SmallVec<[accesskit::Node; 16]>,
    all_nodes: Vec<(NodeId, accesskit::Node)>,
    seen_ids: FxHashSet<NodeId>,
    focus: NodeId,
    #[cfg(debug_assertions)]
    has_set_focus: bool,
}

impl A11yNodeBuilder {
    fn new() -> Self {
        Self {
            ids_stack: SmallVec::new(),
            nodes_stack: SmallVec::new(),
            all_nodes: Vec::new(),
            seen_ids: FxHashSet::default(),
            focus: ROOT_NODE_ID,
            #[cfg(debug_assertions)]
            has_set_focus: false,
        }
    }

    pub(crate) fn push(&mut self, id: NodeId, node: accesskit::Node) -> bool {
        debug_assert!(!self.ids_stack.is_empty(), "push called before push_root");

        if !self.seen_ids.insert(id) {
            debug_assert!(
                false,
                "Duplicate a11y node id: {id:?}. In a release build, this node would be silently discarded from the a11y tree."
            );
            return false;
        }

        if let Some(parent) = self.nodes_stack.last_mut() {
            parent.push_child(id);
        }
        self.ids_stack.push(id);
        self.nodes_stack.push(node);
        true
    }

    pub(crate) fn pop(&mut self) {
        debug_assert!(self.ids_stack.len() > 1, "pop would remove the root node");

        if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
            self.all_nodes.push((id, node));
        }
    }

    pub(crate) fn checkpoint(&self) -> A11yCheckpoint {
        A11yCheckpoint {
            nodes: self.all_nodes.len(),
            children: self.nodes_stack.last().map_or(0, |node| node.children().len()),
            focus: self.focus,
            #[cfg(debug_assertions)]
            has_set_focus: self.has_set_focus,
        }
    }

    pub(crate) fn rollback(&mut self, checkpoint: A11yCheckpoint) {
        for (id, _) in self.all_nodes.drain(checkpoint.nodes..) {
            self.seen_ids.remove(&id);
        }
        if let Some(parent) = self.nodes_stack.last_mut() {
            let children = parent.children()[..checkpoint.children].to_vec();
            parent.set_children(children);
        }
        self.focus = checkpoint.focus;
        #[cfg(debug_assertions)]
        {
            self.has_set_focus = checkpoint.has_set_focus;
        }
    }

    fn begin_frame(&mut self, window_title: Option<&SharedString>) {
        self.all_nodes.clear();
        self.ids_stack.clear();
        self.nodes_stack.clear();
        self.seen_ids.clear();
        #[cfg(debug_assertions)]
        {
            self.has_set_focus = false;
        }
        let mut root_node = accesskit::Node::new(accesskit::Role::Window);
        if let Some(title) = window_title {
            root_node.set_label(title.to_string());
        }

        self.ids_stack.push(ROOT_NODE_ID);
        self.nodes_stack.push(root_node);
        self.focus = ROOT_NODE_ID;
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.ids_stack.is_empty() && self.all_nodes.is_empty()
    }

    pub(crate) fn has_node(&self, id: NodeId) -> bool {
        id == ROOT_NODE_ID || self.seen_ids.contains(&id)
    }

    pub(crate) fn set_focus(&mut self, id: NodeId) {
        #[cfg(debug_assertions)]
        {
            debug_assert!(
                !self.has_set_focus,
                "set_focus called more than once in a single frame"
            );
            self.has_set_focus = true;
        }
        self.focus = id;
    }

    fn finalize(&mut self) -> TreeUpdate {
        debug_assert_eq!(self.ids_stack.len(), 1);
        debug_assert_eq!(self.ids_stack[0], ROOT_NODE_ID);

        if self.ids_stack.len() != 1 {
            log::error!(
                "a11y: Stack imbalance at end of frame: expected 1 (root), got {}. \
                 Some elements may have pushed without popping.",
                self.ids_stack.len()
            );
        }

        while !self.ids_stack.is_empty() {
            if let (Some(id), Some(node)) = (self.ids_stack.pop(), self.nodes_stack.pop()) {
                self.all_nodes.push((id, node));
            }
        }

        let nodes = std::mem::take(&mut self.all_nodes);
        let update = TreeUpdate {
            nodes,
            tree: Some(accesskit::TreeInfo::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: self.focus,
        };

        Self::repair_tree_update(update)
    }

    fn repair_tree_update(mut update: TreeUpdate) -> TreeUpdate {
        let node_ids: FxHashSet<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();

        if !node_ids.contains(&update.focus) {
            log::error!(
                "a11y: Focused node {:?} is not in the tree ({} nodes). \
                 Falling back to root. This is a bug in the a11y tree builder.",
                update.focus,
                update.nodes.len()
            );
            update.focus = ROOT_NODE_ID;
        }

        for (id, node) in &mut update.nodes {
            let children = node.children();
            let valid: Vec<NodeId> = children
                .iter()
                .copied()
                .filter(|child_id| node_ids.contains(child_id))
                .collect();
            if valid.len() != children.len() {
                log::error!(
                    "a11y: Node {:?} references {} children not present in the tree. \
                     Stripping invalid child references.",
                    id,
                    children.len() - valid.len()
                );
                node.set_children(valid);
            }
        }

        update
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> accesskit::Node {
        accesskit::Node::new(accesskit::Role::Button)
    }

    #[test]
    fn nested_nodes_become_children_of_the_labelled_root() {
        let mut builder = A11yNodeBuilder::new();
        builder.begin_frame(Some(&"Window title".into()));
        assert!(builder.push(NodeId(1), node()));
        assert!(builder.push(NodeId(2), node()));
        builder.pop();
        builder.pop();
        builder.set_focus(NodeId(2));

        let update = builder.finalize();
        let find = |id| {
            update
                .nodes
                .iter()
                .find(|(node_id, _)| *node_id == id)
                .map(|(_, node)| node)
                .unwrap()
        };

        assert_eq!(update.focus, NodeId(2));
        assert_eq!(find(ROOT_NODE_ID).label(), Some("Window title"));
        assert_eq!(find(ROOT_NODE_ID).children(), &[NodeId(1)]);
        assert_eq!(find(NodeId(1)).children(), &[NodeId(2)]);
    }

    #[test]
    fn a_rolled_back_prepaint_can_push_the_same_nodes_again() {
        let mut builder = A11yNodeBuilder::new();
        builder.begin_frame(None);
        assert!(builder.push(NodeId(1), node()));
        builder.pop();

        let checkpoint = builder.checkpoint();
        assert!(builder.push(NodeId(2), node()));
        assert!(builder.push(NodeId(3), node()));
        builder.set_focus(NodeId(3));
        builder.pop();
        builder.pop();
        builder.rollback(checkpoint);

        assert!(!builder.has_node(NodeId(2)));
        assert!(builder.has_node(NodeId(1)));
        assert!(builder.push(NodeId(2), node()));
        builder.set_focus(NodeId(2));
        builder.pop();

        let update = builder.finalize();
        let root = update
            .nodes
            .iter()
            .find(|(id, _)| *id == ROOT_NODE_ID)
            .map(|(_, node)| node)
            .unwrap();
        assert_eq!(root.children(), &[NodeId(1), NodeId(2)]);
        assert_eq!(update.focus, NodeId(2));
        assert_eq!(update.nodes.len(), 3);
    }

    #[test]
    fn repair_points_focus_at_the_root_and_drops_missing_children() {
        let mut root = accesskit::Node::new(accesskit::Role::Window);
        root.push_child(NodeId(1));
        root.push_child(NodeId(2));
        let update = A11yNodeBuilder::repair_tree_update(TreeUpdate {
            nodes: vec![(ROOT_NODE_ID, root), (NodeId(1), node())],
            tree: Some(accesskit::TreeInfo::new(ROOT_NODE_ID)),
            tree_id: accesskit::TreeId::ROOT,
            focus: NodeId(9),
        });

        assert_eq!(update.focus, ROOT_NODE_ID);
        assert_eq!(update.nodes[0].1.children(), &[NodeId(1)]);
    }
}

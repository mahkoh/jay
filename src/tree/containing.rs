use {
    crate::tree::{FloatNode, Node, ToplevelNode, WorkspaceNode, transaction::TreeTransaction},
    std::rc::Rc,
};

pub trait ContainingNode: Node {
    fn cnode_replace_child(
        self: Rc<Self>,
        tt: &TreeTransaction,
        old: &dyn Node,
        new: Rc<dyn ToplevelNode>,
    );
    fn cnode_remove_child(self: Rc<Self>, tt: &TreeTransaction, child: &dyn Node) {
        self.cnode_remove_child2(tt, child, false);
    }
    fn cnode_remove_child2(
        self: Rc<Self>,
        tt: &TreeTransaction,
        child: &dyn Node,
        preserve_focus: bool,
    );
    fn cnode_accepts_child(&self, node: &dyn Node) -> bool;
    fn cnode_child_attention_request_changed(self: Rc<Self>, child: &dyn Node, set: bool);
    fn cnode_workspace(self: Rc<Self>) -> Rc<WorkspaceNode>;
    fn cnode_make_visible(self: Rc<Self>, tt: &TreeTransaction, child: &dyn Node);
    fn cnode_set_child_position(
        self: Rc<Self>,
        tt: &TreeTransaction,
        child: &dyn Node,
        x: i32,
        y: i32,
    ) {
        let _ = tt;
        let _ = child;
        let _ = x;
        let _ = y;
    }
    fn cnode_resize_child(
        self: Rc<Self>,
        tt: &TreeTransaction,
        child: &dyn Node,
        new_x1: Option<i32>,
        new_y1: Option<i32>,
        new_x2: Option<i32>,
        new_y2: Option<i32>,
    ) {
        let _ = tt;
        let _ = child;
        let _ = new_x1;
        let _ = new_x2;
        let _ = new_y1;
        let _ = new_y2;
    }
    fn cnode_pinned(&self) -> bool {
        false
    }
    fn cnode_set_pinned(self: Rc<Self>, pinned: bool) {
        let _ = pinned;
    }
    fn cnode_get_float(self: Rc<Self>) -> Option<Rc<FloatNode>> {
        None
    }
    fn cnode_self_or_ancestor_fullscreen(&self) -> bool {
        false
    }
}

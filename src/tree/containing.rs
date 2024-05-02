use {
    crate::tree::{Node, ToplevelNode, WorkspaceNode},
    std::rc::Rc,
};

pub trait ContainingNode: Node {
    fn cnode_replace_child(self: Rc<Self>, old: &dyn Node, new: Rc<dyn ToplevelNode>);
    fn cnode_remove_child(self: Rc<Self>, child: &dyn Node) {
        self.cnode_remove_child2(child, false);
    }
    fn cnode_remove_child2(self: Rc<Self>, child: &dyn Node, preserve_focus: bool);
    fn cnode_accepts_child(&self, node: &dyn Node) -> bool;
    fn cnode_child_attention_request_changed(self: Rc<Self>, child: &dyn Node, set: bool);
    fn cnode_workspace(self: Rc<Self>) -> Rc<WorkspaceNode>;
}

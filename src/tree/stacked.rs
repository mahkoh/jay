use {
    crate::tree::{Node, WorkspaceNode},
    std::rc::Rc,
};

pub trait StackedNode: Node {
    fn stacked_prepare_set_visible(&self) {
        // nothing
    }
    fn stacked_needs_set_visible(&self) -> bool {
        true
    }
    fn stacked_set_visible(&self, visible: bool);
    fn stacked_has_workspace_link(&self) -> bool;

    fn stacked_absolute_position_constrains_input(&self) -> bool {
        true
    }

    fn stacked_is_xdg_popup(&self) -> bool {
        false
    }
}

pub trait PinnedNode: StackedNode {
    fn set_workspace(self: Rc<Self>, workspace: &Rc<WorkspaceNode>, update_visible: bool);
}

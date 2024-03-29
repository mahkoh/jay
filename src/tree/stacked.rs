use {crate::tree::Node, std::rc::Rc};

pub trait StackedNode: Node {
    fn stacked_as_node(&self) -> &dyn Node;
    fn stacked_into_node(self: Rc<Self>) -> Rc<dyn Node>;
    fn stacked_into_dyn(self: Rc<Self>) -> Rc<dyn StackedNode>;
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
}

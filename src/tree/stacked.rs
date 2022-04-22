use {crate::tree::Node, std::rc::Rc};

pub trait StackedNode: Node {
    fn stacked_as_node(&self) -> &dyn Node;
    fn stacked_into_node(self: Rc<Self>) -> Rc<dyn Node>;
    fn stacked_into_dyn(self: Rc<Self>) -> Rc<dyn StackedNode>;

    fn stacked_absolute_position_constrains_input(&self) -> bool {
        true
    }
}

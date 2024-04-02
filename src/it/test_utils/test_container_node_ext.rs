use {
    crate::{
        it::test_error::TestResult,
        tree::{ContainerNode, ToplevelNode},
    },
    std::rc::Rc,
};

pub trait TestContainerExt {
    fn first_toplevel(&self) -> TestResult<Rc<dyn ToplevelNode>>;
}

impl TestContainerExt for ContainerNode {
    fn first_toplevel(&self) -> TestResult<Rc<dyn ToplevelNode>> {
        match self.children.first() {
            None => bail!("container does not have children"),
            Some(c) => Ok(c.node.clone()),
        }
    }
}

use {
    crate::{
        it::test_error::TestResult,
        tree::{ContainerNode, WorkspaceNode},
    },
    std::rc::Rc,
};

pub trait TestWorkspaceNodeExt {
    fn container(&self) -> TestResult<Rc<ContainerNode>>;
}

impl TestWorkspaceNodeExt for WorkspaceNode {
    fn container(&self) -> TestResult<Rc<ContainerNode>> {
        match self.current.container.get() {
            None => bail!("workspace does not have a container"),
            Some(c) => Ok(c),
        }
    }
}

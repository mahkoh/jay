use {
    crate::{
        it::test_error::TestResult,
        tree::{ContainerNode, TreeTimeline::LiveTL, WorkspaceNode},
    },
    std::rc::Rc,
};

pub trait TestWorkspaceNodeExt {
    fn container(&self) -> TestResult<Rc<ContainerNode>>;
}

impl TestWorkspaceNodeExt for WorkspaceNode {
    fn container(&self) -> TestResult<Rc<ContainerNode>> {
        match self.node_state[LiveTL].container.get() {
            None => bail!("workspace does not have a container"),
            Some(c) => Ok(c),
        }
    }
}

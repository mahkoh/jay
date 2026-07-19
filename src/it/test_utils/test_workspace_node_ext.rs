use crate::it::test_error::TestResult;
use crate::tree::ContainerNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::WorkspaceNode;
use std::rc::Rc;

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

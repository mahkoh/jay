use crate::it::test_error::TestResult;
use crate::it::test_utils::test_container_node_ext::TestContainerExt;
use crate::it::test_utils::test_workspace_node_ext::TestWorkspaceNodeExt;
use crate::tree::OutputNode;
use crate::tree::ToplevelNode;
use crate::tree::TreeTimeline::LiveTL;
use crate::tree::WorkspaceNode;
use std::rc::Rc;

pub trait TestOutputNodeExt {
    fn workspace2(&self) -> TestResult<Rc<WorkspaceNode>>;
    fn first_toplevel(&self) -> TestResult<Rc<dyn ToplevelNode>>;
}

impl TestOutputNodeExt for OutputNode {
    fn workspace2(&self) -> TestResult<Rc<WorkspaceNode>> {
        match self.node_state[LiveTL].workspace.get() {
            None => bail!("Output node does not have a container"),
            Some(w) => Ok(w),
        }
    }

    fn first_toplevel(&self) -> TestResult<Rc<dyn ToplevelNode>> {
        self.workspace2()?.container()?.first_toplevel()
    }
}

use {
    crate::{
        it::{
            test_error::TestResult,
            test_utils::{
                test_container_node_ext::TestContainerExt,
                test_workspace_node_ext::TestWorkspaceNodeExt,
            },
        },
        tree::{OutputNode, ToplevelNode, WorkspaceNode},
    },
    std::rc::Rc,
};

pub trait TestOutputNodeExt {
    fn workspace(&self) -> TestResult<Rc<WorkspaceNode>>;
    fn first_toplevel(&self) -> TestResult<Rc<dyn ToplevelNode>>;
}

impl TestOutputNodeExt for OutputNode {
    fn workspace(&self) -> TestResult<Rc<WorkspaceNode>> {
        match self.workspace.get() {
            None => bail!("Output node does not have a container"),
            Some(w) => Ok(w),
        }
    }

    fn first_toplevel(&self) -> TestResult<Rc<dyn ToplevelNode>> {
        self.workspace()?.container()?.first_toplevel()
    }
}

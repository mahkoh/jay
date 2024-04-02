use {
    crate::{
        it::test_error::TestResult,
        tree::{OutputNode, WorkspaceNode},
    },
    std::rc::Rc,
};

pub trait TestOutputNodeExt {
    fn workspace(&self) -> TestResult<Rc<WorkspaceNode>>;
}

impl TestOutputNodeExt for OutputNode {
    fn workspace(&self) -> TestResult<Rc<WorkspaceNode>> {
        match self.workspace.get() {
            None => bail!("Output node does not have a container"),
            Some(w) => Ok(w),
        }
    }
}

use crate::it::test_error::TestErrorError;
use crate::wire::ExtWorkspaceGroupHandleV1Id;
use crate::wire::ext_workspace_group_handle_v1::*;
use std::rc::Rc;

pub struct TestExtWorkspaceGroupHandle {
    #[expect(unused)]
    pub id: ExtWorkspaceGroupHandleV1Id,
}

synthetic_event_handler!(TestExtWorkspaceGroupHandle);

impl ExtWorkspaceGroupHandleV1EventHandler for TestExtWorkspaceGroupHandle {
    type Error = TestErrorError;

    fn capabilities(&self, _ev: Capabilities, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn output_enter(&self, _ev: OutputEnter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn output_leave(&self, _ev: OutputLeave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn workspace_enter(&self, _ev: WorkspaceEnter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn workspace_leave(&self, _ev: WorkspaceLeave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn removed(&self, _ev: Removed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

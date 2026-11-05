use crate::it::test_error::TestErrorError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_ext_workspace_manager::TestExtWorkspaceManager;
use crate::wire::ExtWorkspaceGroupHandleV1Id;
use crate::wire::ExtWorkspaceHandleV1Id;
use crate::wire::ObjectId;
use crate::wire::ext_workspace_group_handle_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::rc::Weak;

pub struct TestExtWorkspaceGroupHandle {
    pub id: ExtWorkspaceGroupHandleV1Id,
    pub manager: Weak<TestExtWorkspaceManager>,
    pub removed: Cell<bool>,
    pub capabilities: Cell<u32>,
    pub outputs: RefCell<Vec<ObjectId>>,
    pub workspaces: RefCell<Vec<ObjectId>>,
}

impl TestExtWorkspaceGroupHandle {
    pub fn create_workspace(&self, name: &str) -> TestResult {
        let Some(manager) = self.manager.upgrade() else {
            bail!("Workspace manager is gone");
        };
        manager.client.request(CreateWorkspace {
            self_id: self.id,
            workspace: name,
        });
        Ok(())
    }

    fn set_workspace_group(
        &self,
        workspace_id: ExtWorkspaceHandleV1Id,
        group: Option<ExtWorkspaceGroupHandleV1Id>,
    ) {
        let Some(manager) = self.manager.upgrade() else {
            return;
        };
        let Some(workspace) = manager.workspace_by_id(workspace_id) else {
            return;
        };
        workspace.current_group.set(group.map(Into::into));
    }
}

synthetic_event_handler!(TestExtWorkspaceGroupHandle);

impl ExtWorkspaceGroupHandleV1EventHandler for TestExtWorkspaceGroupHandle {
    type Error = TestErrorError;

    fn capabilities(&self, ev: Capabilities, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.capabilities.set(ev.capabilities);
        Ok(())
    }

    fn output_enter(&self, ev: OutputEnter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = ev.output.into();
        let mut outputs = self.outputs.borrow_mut();
        if !outputs.contains(&output) {
            outputs.push(output);
        }
        Ok(())
    }

    fn output_leave(&self, ev: OutputLeave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let output = ev.output.into();
        self.outputs.borrow_mut().retain(|id| *id != output);
        Ok(())
    }

    fn workspace_enter(&self, ev: WorkspaceEnter, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let workspace = ev.workspace.into();
        {
            let mut workspaces = self.workspaces.borrow_mut();
            if !workspaces.contains(&workspace) {
                workspaces.push(workspace);
            }
        }
        self.set_workspace_group(ev.workspace, Some(self.id));
        Ok(())
    }

    fn workspace_leave(&self, ev: WorkspaceLeave, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let workspace = ev.workspace.into();
        self.workspaces.borrow_mut().retain(|id| *id != workspace);
        self.set_workspace_group(ev.workspace, None);
        Ok(())
    }

    fn removed(&self, _ev: Removed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.removed.set(true);
        Ok(())
    }
}

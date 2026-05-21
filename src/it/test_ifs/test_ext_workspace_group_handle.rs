use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_ifs::test_ext_workspace_manager::TestExtWorkspaceManager,
            test_object::TestObject,
            testrun::ParseFull,
        },
        object::ObjectId,
        utils::buffd::MsgParser,
        wire::{
            ExtWorkspaceGroupHandleV1Id, ExtWorkspaceHandleV1Id, ext_workspace_group_handle_v1::*,
        },
    },
    std::{
        cell::{Cell, RefCell},
        rc::Weak,
    },
};

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
        manager.tran.send(CreateWorkspace {
            self_id: self.id,
            workspace: name,
        })?;
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

    fn handle_capabilities(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Capabilities::parse_full(parser)?;
        self.capabilities.set(ev.capabilities);
        Ok(())
    }

    fn handle_output_enter(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = OutputEnter::parse_full(parser)?;
        let output = ev.output.into();
        let mut outputs = self.outputs.borrow_mut();
        if !outputs.contains(&output) {
            outputs.push(output);
        }
        Ok(())
    }

    fn handle_output_leave(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = OutputLeave::parse_full(parser)?;
        let output = ev.output.into();
        self.outputs.borrow_mut().retain(|id| *id != output);
        Ok(())
    }

    fn handle_workspace_enter(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = WorkspaceEnter::parse_full(parser)?;
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

    fn handle_workspace_leave(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = WorkspaceLeave::parse_full(parser)?;
        let workspace = ev.workspace.into();
        self.workspaces.borrow_mut().retain(|id| *id != workspace);
        self.set_workspace_group(ev.workspace, None);
        Ok(())
    }

    fn handle_removed(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Removed::parse_full(parser)?;
        self.removed.set(true);
        Ok(())
    }
}

test_object! {
    TestExtWorkspaceGroupHandle, ExtWorkspaceGroupHandleV1;

    CAPABILITIES => handle_capabilities,
    OUTPUT_ENTER => handle_output_enter,
    OUTPUT_LEAVE => handle_output_leave,
    WORKSPACE_ENTER => handle_workspace_enter,
    WORKSPACE_LEAVE => handle_workspace_leave,
    REMOVED => handle_removed,
}

impl TestObject for TestExtWorkspaceGroupHandle {}

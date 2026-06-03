use {
    crate::{
        it::{
            test_error::{TestError, TestResult},
            test_ifs::test_ext_workspace_group_handle::TestExtWorkspaceGroupHandle,
            test_object::TestObject,
            test_transport::TestTransport,
            testrun::ParseFull,
        },
        object::ObjectId,
        utils::buffd::MsgParser,
        wire::{ExtWorkspaceHandleV1Id, ext_workspace_handle_v1::*},
    },
    std::{
        cell::{Cell, RefCell},
        rc::Rc,
    },
};

pub struct TestExtWorkspaceHandle {
    pub id: ExtWorkspaceHandleV1Id,
    pub tran: Rc<TestTransport>,
    pub removed: Cell<bool>,
    pub state: Cell<u32>,
    pub capabilities: Cell<u32>,
    pub identifier: RefCell<Option<String>>,
    pub name: RefCell<Option<String>>,
    pub current_group: Cell<Option<ObjectId>>,
}

impl TestExtWorkspaceHandle {
    pub fn activate(&self) -> TestResult {
        self.tran.send(Activate { self_id: self.id })?;
        Ok(())
    }

    pub fn assign(&self, group: &TestExtWorkspaceGroupHandle) -> TestResult {
        self.tran.send(Assign {
            self_id: self.id,
            workspace_group: group.id,
        })?;
        Ok(())
    }

    fn handle_id(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Id::parse_full(parser)?;
        *self.identifier.borrow_mut() = Some(ev.id.to_string());
        Ok(())
    }

    fn handle_name(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Name::parse_full(parser)?;
        *self.name.borrow_mut() = Some(ev.name.to_string());
        Ok(())
    }

    fn handle_state(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = State::parse_full(parser)?;
        self.state.set(ev.state);
        Ok(())
    }

    fn handle_capabilities(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let ev = Capabilities::parse_full(parser)?;
        self.capabilities.set(ev.capabilities);
        Ok(())
    }

    fn handle_removed(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Removed::parse_full(parser)?;
        self.removed.set(true);
        self.current_group.set(None);
        Ok(())
    }
}

test_object! {
    TestExtWorkspaceHandle, ExtWorkspaceHandleV1;

    ID => handle_id,
    NAME => handle_name,
    STATE => handle_state,
    CAPABILITIES => handle_capabilities,
    REMOVED => handle_removed,
}

impl TestObject for TestExtWorkspaceHandle {}

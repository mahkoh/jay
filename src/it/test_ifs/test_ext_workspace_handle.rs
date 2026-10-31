use crate::client::Client;
use crate::it::test_error::TestErrorError;
use crate::it::test_error::TestResult;
use crate::it::test_ifs::test_ext_workspace_group_handle::TestExtWorkspaceGroupHandle;
use crate::wire::ExtWorkspaceHandleV1Id;
use crate::wire::ObjectId;
use crate::wire::ext_workspace_handle_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestExtWorkspaceHandle {
    pub id: ExtWorkspaceHandleV1Id,
    pub client: Rc<Client>,
    pub removed: Cell<bool>,
    pub state: Cell<u32>,
    pub capabilities: Cell<u32>,
    pub identifier: RefCell<Option<String>>,
    pub name: RefCell<Option<String>>,
    pub current_group: Cell<Option<ObjectId>>,
}

impl TestExtWorkspaceHandle {
    pub fn activate(&self) -> TestResult {
        self.client.request(Activate { self_id: self.id });
        Ok(())
    }

    pub fn assign(&self, group: &TestExtWorkspaceGroupHandle) -> TestResult {
        self.client.request(Assign {
            self_id: self.id,
            workspace_group: group.id,
        });
        Ok(())
    }
}

synthetic_event_handler!(TestExtWorkspaceHandle);

impl ExtWorkspaceHandleV1EventHandler for TestExtWorkspaceHandle {
    type Error = TestErrorError;

    fn id_(&self, ev: Id<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.identifier.borrow_mut() = Some(ev.id.to_string());
        Ok(())
    }

    fn name(&self, ev: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.name.borrow_mut() = Some(ev.name.to_string());
        Ok(())
    }

    fn coordinates(&self, _ev: Coordinates<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn state(&self, ev: State, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.state.set(ev.state);
        Ok(())
    }

    fn capabilities(&self, ev: Capabilities, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.capabilities.set(ev.capabilities);
        Ok(())
    }

    fn removed(&self, _ev: Removed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.removed.set(true);
        self.current_group.set(None);
        Ok(())
    }
}

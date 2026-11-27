use crate::it::test_error::TestErrorError;
use crate::wire::ExtWorkspaceHandleV1Id;
use crate::wire::ext_workspace_handle_v1::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestExtWorkspaceHandle {
    #[expect(unused)]
    pub id: ExtWorkspaceHandleV1Id,
    pub name: RefCell<String>,
    pub coordinates: RefCell<Vec<u32>>,
    pub removed: Cell<bool>,
}

synthetic_event_handler!(TestExtWorkspaceHandle);

impl ExtWorkspaceHandleV1EventHandler for TestExtWorkspaceHandle {
    type Error = TestErrorError;

    fn id_(&self, _ev: Id<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn name(&self, ev: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.name.borrow_mut() = ev.name.to_string();
        Ok(())
    }

    fn coordinates(&self, ev: Coordinates<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.coordinates.borrow_mut() = ev.coordinates.to_vec();
        Ok(())
    }

    fn state(&self, _ev: State, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn capabilities(&self, _ev: Capabilities, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn removed(&self, _ev: Removed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.removed.set(true);
        Ok(())
    }
}

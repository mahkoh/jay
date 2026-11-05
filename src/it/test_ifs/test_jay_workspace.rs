use crate::it::test_error::TestErrorError;
use crate::wire::JayWorkspaceId;
use crate::wire::jay_workspace::*;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct TestJayWorkspace {
    pub id: JayWorkspaceId,
    pub destroyed: Cell<bool>,
    pub linear_id: Cell<Option<u32>>,
    pub name: RefCell<Option<String>>,
    pub output: Cell<Option<u32>>,
    pub visible: Cell<Option<bool>>,
}

synthetic_event_handler!(TestJayWorkspace);

impl JayWorkspaceEventHandler for TestJayWorkspace {
    type Error = TestErrorError;

    fn linear_id(&self, ev: LinearId, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.linear_id.set(Some(ev.linear_id));
        Ok(())
    }

    fn name(&self, ev: Name<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        *self.name.borrow_mut() = Some(ev.name.to_string());
        Ok(())
    }

    fn destroyed(&self, _ev: Destroyed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.destroyed.set(true);
        Ok(())
    }

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn output(&self, ev: Output, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.output.set(Some(ev.global_name));
        Ok(())
    }

    fn visible(&self, ev: Visible, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.visible.set(Some(ev.visible));
        Ok(())
    }
}

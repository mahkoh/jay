use crate::client::Client;
use crate::it::test_error::TestErrorError;
use crate::wire::ExtForeignToplevelHandleV1Id;
use crate::wire::ext_foreign_toplevel_handle_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestExtForeignToplevelHandle {
    pub id: ExtForeignToplevelHandleV1Id,
    pub client: Rc<Client>,
    pub closed: Cell<bool>,
    pub title: Cell<Option<String>>,
    pub app_id: Cell<Option<String>>,
    pub identifier: Cell<Option<String>>,
}

impl TestExtForeignToplevelHandle {
    fn destroy(&self) {
        self.client
            .send_ext_foreign_toplevel_handle_v1_destroy(self.id);
    }
}

synthetic_event_handler!(TestExtForeignToplevelHandle);

impl ExtForeignToplevelHandleV1EventHandler for TestExtForeignToplevelHandle {
    type Error = TestErrorError;

    fn closed(&self, _ev: Closed, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.closed.set(true);
        self.destroy();
        Ok(())
    }

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn title(&self, ev: Title<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.title.set(Some(ev.title.to_string()));
        Ok(())
    }

    fn app_id(&self, ev: AppId<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.app_id.set(Some(ev.app_id.to_string()));
        Ok(())
    }

    fn identifier(&self, ev: Identifier<'_>, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.identifier.set(Some(ev.identifier.to_string()));
        Ok(())
    }
}

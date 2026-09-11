use crate::client::Client;
use crate::it::test_error::TestErrorError;
use crate::wire::XdgToplevelSessionV1Id;
use crate::wire::xdg_toplevel_session_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestToplevelSession {
    pub id: XdgToplevelSessionV1Id,
    pub client: Rc<Client>,
    pub restored: Cell<bool>,
}

impl TestToplevelSession {
    pub fn destroy(&self) {
        self.client.send_xdg_toplevel_session_v1_destroy(self.id);
    }

    #[expect(unused)]
    pub fn rename(&self, name: &str) {
        self.client
            .send_xdg_toplevel_session_v1_rename(self.id, name);
    }
}

synthetic_event_handler!(TestToplevelSession);

impl XdgToplevelSessionV1EventHandler for TestToplevelSession {
    type Error = TestErrorError;

    fn restored(&self, _ev: Restored, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.restored.set(true);
        Ok(())
    }
}

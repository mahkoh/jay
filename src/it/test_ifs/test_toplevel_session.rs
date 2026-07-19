use crate::it::test_error::TestError;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::it::testrun::ParseFull;
use crate::utils::buffd::MsgParser;
use crate::wire::XdgToplevelSessionV1Id;
use crate::wire::xdg_toplevel_session_v1::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestToplevelSession {
    pub id: XdgToplevelSessionV1Id,
    pub tran: Rc<TestTransport>,
    pub destroyed: Cell<bool>,
    pub restored: Cell<bool>,
}

impl TestToplevelSession {
    pub fn destroy(&self) -> Result<(), TestError> {
        if !self.destroyed.replace(true) {
            self.tran.send(Destroy { self_id: self.id })?;
        }
        Ok(())
    }

    #[expect(dead_code)]
    pub fn rename(&self, name: &str) -> Result<(), TestError> {
        self.tran.send(Rename {
            self_id: self.id,
            name,
        })?;
        Ok(())
    }

    fn handle_restored(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Restored::parse_full(parser)?;
        self.restored.set(true);
        Ok(())
    }
}

test_object! {
    TestToplevelSession, XdgToplevelSessionV1;

    RESTORED => handle_restored,
}

impl TestObject for TestToplevelSession {}

impl Drop for TestToplevelSession {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

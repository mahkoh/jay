use {
    crate::{
        it::{
            test_error::TestError, test_object::TestObject, test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{XdgToplevelSessionV1Id, xdg_toplevel_session_v1::*},
    },
    std::{cell::Cell, rc::Rc},
};

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

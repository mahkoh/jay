use crate::it::test_error::TestError;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::it::testrun::ParseFull;
use crate::utils::buffd::MsgParser;
use crate::wire::WlBufferId;
use crate::wire::wl_buffer::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestBuffer {
    pub id: WlBufferId,
    pub tran: Rc<TestTransport>,
    pub released: Cell<bool>,
    pub destroyed: Cell<bool>,
}

impl TestBuffer {
    pub fn destroy(&self) -> Result<(), TestError> {
        if self.destroyed.replace(true) {
            return Ok(());
        }
        self.tran.send(Destroy { self_id: self.id })?;
        Ok(())
    }

    fn handle_release(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Release::parse_full(parser)?;
        self.released.set(true);
        Ok(())
    }
}

impl Drop for TestBuffer {
    fn drop(&mut self) {
        let _ = self.destroy();
    }
}

test_object! {
    TestBuffer, WlBuffer;

    RELEASE => handle_release,
}

impl TestObject for TestBuffer {}

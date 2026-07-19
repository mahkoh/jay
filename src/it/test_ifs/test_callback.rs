use crate::it::test_error::TestError;
use crate::it::test_object::TestObject;
use crate::it::test_transport::TestTransport;
use crate::it::testrun::ParseFull;
use crate::utils::buffd::MsgParser;
use crate::wire::WlCallbackId;
use crate::wire::wl_callback::*;
use std::cell::Cell;
use std::rc::Rc;

pub struct TestCallback {
    pub id: WlCallbackId,
    pub _tran: Rc<TestTransport>,
    pub handler: Cell<Option<Box<dyn FnOnce()>>>,
    pub done: Cell<bool>,
}

impl TestCallback {
    fn handle_done(&self, parser: MsgParser<'_, '_>) -> Result<(), TestError> {
        let _ev = Done::parse_full(parser)?;
        self.dispatch();
        Ok(())
    }

    fn dispatch(&self) {
        self.done.set(true);
        if let Some(handler) = self.handler.take() {
            handler();
        }
    }
}

test_object! {
    TestCallback, WlCallback;

    DONE => handle_done,
}

impl TestObject for TestCallback {
    fn on_remove(&self, _transport: &TestTransport) {
        self.dispatch();
    }
}

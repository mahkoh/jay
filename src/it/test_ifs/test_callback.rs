use {
    crate::{
        it::{
            test_error::TestError, test_object::TestObject, test_transport::TestTransport,
            testrun::ParseFull,
        },
        utils::buffd::MsgParser,
        wire::{wl_callback::*, WlCallbackId},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct TestCallback {
    pub id: WlCallbackId,
    pub tran: Rc<TestTransport>,
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

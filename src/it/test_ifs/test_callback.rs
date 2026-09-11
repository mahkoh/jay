use crate::it::test_error::TestErrorError;
use crate::wire::wl_callback::*;
use std::cell::Cell;
use std::rc::Rc;

#[derive(Default)]
pub struct TestCallback {
    pub handler: Cell<Option<Box<dyn FnOnce()>>>,
    pub done: Cell<bool>,
}

impl TestCallback {
    fn dispatch(&self) {
        self.done.set(true);
        if let Some(handler) = self.handler.take() {
            handler();
        }
    }
}

synthetic_event_handler!(TestCallback);

impl WlCallbackEventHandler for TestCallback {
    type Error = TestErrorError;

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.dispatch();
        Ok(())
    }
}

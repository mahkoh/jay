use {
    crate::{
        object::Version,
        wire::{WlCallbackId, wl_callback::*},
        wl_usr::{UsrCon, usr_object::UsrObject},
    },
    std::{cell::Cell, convert::Infallible, rc::Rc},
};

pub struct UsrWlCallback {
    pub id: WlCallbackId,
    pub con: Rc<UsrCon>,
    pub handler: Cell<Option<Box<dyn FnOnce()>>>,
    pub version: Version,
}

impl UsrWlCallback {
    pub fn new<F>(con: &Rc<UsrCon>, handler: F) -> Self
    where
        F: FnOnce() + 'static,
    {
        Self {
            id: con.id(),
            con: con.clone(),
            handler: Cell::new(Some(Box::new(handler))),
            version: Version(1),
        }
    }
}

impl WlCallbackEventHandler for UsrWlCallback {
    type Error = Infallible;

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(handler) = self.handler.take() {
            handler();
        }
        self.con.remove_obj(self);
        Ok(())
    }
}

usr_object_base! {
    self = UsrWlCallback = WlCallback;
    version = self.version;
}

impl UsrObject for UsrWlCallback {
    fn destroy(&self) {
        // nothing
    }

    fn break_loops(&self) {
        self.handler.take();
    }
}

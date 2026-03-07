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
    pub owner: Cell<Option<Rc<dyn UsrWlCallbackOwner>>>,
    pub version: Version,
}

pub trait UsrWlCallbackOwner {
    fn done(self: Rc<Self>);
}

impl<T> UsrWlCallbackOwner for Cell<Option<T>>
where
    T: FnOnce() + 'static,
{
    fn done(self: Rc<Self>) {
        if let Some(slf) = self.take() {
            slf();
        }
    }
}

impl UsrWlCallback {
    pub fn new(con: &Rc<UsrCon>) -> Self {
        Self {
            id: con.id(),
            con: con.clone(),
            owner: Default::default(),
            version: Version(1),
        }
    }
}

impl WlCallbackEventHandler for UsrWlCallback {
    type Error = Infallible;

    fn done(&self, _ev: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        if let Some(handler) = self.owner.take() {
            handler.done();
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
        self.owner.take();
    }
}

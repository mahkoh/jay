use {
    crate::{
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_callback::*, WlCallbackId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{cell::Cell, rc::Rc},
};

pub struct UsrWlCallback {
    pub id: WlCallbackId,
    pub con: Rc<UsrCon>,
    pub handler: Cell<Option<Box<dyn FnOnce()>>>,
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
        }
    }

    fn done(&self, parser: MsgParser<'_, '_>) -> Result<(), MsgParserError> {
        let _ev: Done = self.con.parse(self, parser)?;
        if let Some(handler) = self.handler.take() {
            handler();
        }
        self.con.remove_obj(self);
        Ok(())
    }
}

usr_object_base! {
    UsrWlCallback, WlCallback;

    DONE => done,
}

impl UsrObject for UsrWlCallback {
    fn destroy(&self) {
        // nothing
    }

    fn break_loops(&self) {
        self.handler.take();
    }
}

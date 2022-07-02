use {
    crate::{
        utils::buffd::{MsgParser, MsgParserError},
        wire::{wl_callback::*, WlCallbackId},
        wl_usr::{usr_object::UsrObject, UsrCon},
    },
    std::{cell::Cell, error::Error, rc::Rc},
    thiserror::Error,
};

pub struct UsrWlCallback {
    pub id: WlCallbackId,
    pub con: Rc<UsrCon>,
    pub handler: Cell<Option<Box<dyn FnOnce() -> Result<(), Box<dyn Error>>>>>,
}

impl UsrWlCallback {
    pub fn new<E, F>(con: &Rc<UsrCon>, handler: F) -> Self
    where
        E: std::error::Error + 'static,
        F: FnOnce() -> Result<(), E> + 'static,
    {
        Self {
            id: con.id(),
            con: con.clone(),
            handler: Cell::new(Some(Box::new(move || {
                handler().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
            }))),
        }
    }

    fn done(&self, parser: MsgParser<'_, '_>) -> Result<(), WlCallbackError> {
        let _ev: Done = self.con.parse(self, parser)?;
        let res = match self.handler.take() {
            Some(handler) => handler().map_err(WlCallbackError::CallbackError),
            None => Ok(()),
        };
        self.con.remove_obj(self);
        res
    }
}

usr_object_base! {
    UsrWlCallback, WlCallback;

    DONE => done,
}

impl UsrObject for UsrWlCallback {
    fn break_loops(&self) {
        self.handler.take();
    }
}

#[derive(Debug, Error)]
pub enum WlCallbackError {
    #[error(transparent)]
    MsgParserError(#[from] MsgParserError),
    #[error("The callback returned an error")]
    CallbackError(#[source] Box<dyn std::error::Error>),
}

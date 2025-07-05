use {
    crate::{
        client::{Client, ClientError},
        ifs::head_management::{HeadTransactionResult, jay_head_error_v1::JayHeadErrorV1},
        leaks::Tracker,
        object::{Object, Version},
        wire::{JayHeadTransactionResultV1Id, jay_head_transaction_result_v1::*},
    },
    std::{cell::Cell, rc::Rc},
    thiserror::Error,
};

pub struct JayHeadTransactionResultV1 {
    pub id: JayHeadTransactionResultV1Id,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
    pub result: HeadTransactionResult,
    pub destroyed: Cell<bool>,
}

impl JayHeadTransactionResultV1 {
    pub(super) fn send(&self) {
        match self.result {
            HeadTransactionResult::Success => self.send_success(),
            _ => self.send_failed(),
        }
    }

    fn send_success(&self) {
        self.client.event(Success { self_id: self.id });
    }

    fn send_failed(&self) {
        self.client.event(Failed { self_id: self.id });
    }
}

impl JayHeadTransactionResultV1RequestHandler for JayHeadTransactionResultV1 {
    type Error = JayHeadTransactionResultV1Error;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        self.destroyed.set(true);
        Ok(())
    }

    fn get_error(&self, req: GetError, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        let err = Rc::new(JayHeadErrorV1 {
            id: req.error,
            client: self.client.clone(),
            tracker: Default::default(),
            version: self.version,
        });
        track!(self.client, err);
        self.client.add_client_obj(&err)?;
        Ok(())
    }
}

object_base! {
    self = JayHeadTransactionResultV1;
    version = self.version;
}

impl Object for JayHeadTransactionResultV1 {}

simple_add_obj!(JayHeadTransactionResultV1);

#[derive(Debug, Error)]
pub enum JayHeadTransactionResultV1Error {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayHeadTransactionResultV1Error, ClientError);

use crate::client::Client;
use crate::client::ClientError;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::utils::errorfmt::ErrorFmt;
use crate::wire::JayAcceptorRequestId;
use crate::wire::jay_acceptor_request::*;
use std::error::Error;
use std::rc::Rc;
use thiserror::Error;

pub struct JayAcceptorRequest {
    pub id: JayAcceptorRequestId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayAcceptorRequest {
    pub fn send_done(&self, name: &str) {
        self.client.event(Done {
            self_id: self.id,
            name,
        });
    }

    pub fn send_failed(&self, err: impl Error) {
        let msg = &ErrorFmt(err).to_string();
        self.client.event(Failed {
            self_id: self.id,
            msg,
        });
    }
}

impl JayAcceptorRequestRequestHandler for JayAcceptorRequest {
    type Error = JayAcceptorRequestError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayAcceptorRequest;
    version = self.version;
}

impl Object for JayAcceptorRequest {}

simple_add_obj!(JayAcceptorRequest);

#[derive(Debug, Error)]
pub enum JayAcceptorRequestError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayAcceptorRequestError, ClientError);

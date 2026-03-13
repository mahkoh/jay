use {
    crate::{
        client::{Client, ClientError},
        leaks::Tracker,
        object::{Object, Version},
        utils::errorfmt::ErrorFmt,
        wire::{JayOpenControlCenterRequestId, jay_open_control_center_request::*},
    },
    std::{error::Error, rc::Rc},
    thiserror::Error,
};

pub struct JayOpenControlCenterRequest {
    pub id: JayOpenControlCenterRequestId,
    pub client: Rc<Client>,
    pub tracker: Tracker<Self>,
    pub version: Version,
}

impl JayOpenControlCenterRequest {
    pub fn send_failed(&self, err: impl Error) {
        let msg = &ErrorFmt(err).to_string();
        self.client.event(Failed {
            self_id: self.id,
            msg,
        });
    }
}

impl JayOpenControlCenterRequestRequestHandler for JayOpenControlCenterRequest {
    type Error = JayOpenControlCenterRequestError;

    fn destroy(&self, _req: Destroy, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        self.client.remove_obj(self)?;
        Ok(())
    }
}

object_base! {
    self = JayOpenControlCenterRequest;
    version = self.version;
}

impl Object for JayOpenControlCenterRequest {}

simple_add_obj!(JayOpenControlCenterRequest);

#[derive(Debug, Error)]
pub enum JayOpenControlCenterRequestError {
    #[error(transparent)]
    ClientError(Box<ClientError>),
}
efrom!(JayOpenControlCenterRequestError, ClientError);

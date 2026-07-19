use crate::client::Client;
use crate::client::ClientError;
use crate::leaks::Tracker;
use crate::object::Object;
use crate::object::Version;
use crate::utils::errorfmt::ErrorFmt;
use crate::wire::JayOpenControlCenterRequestId;
use crate::wire::jay_open_control_center_request::*;
use std::error::Error;
use std::rc::Rc;
use thiserror::Error;

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

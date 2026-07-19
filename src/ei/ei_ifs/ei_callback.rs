use crate::ei::ei_client::EiClient;
use crate::ei::ei_client::EiClientError;
use crate::ei::ei_object::EiObject;
use crate::ei::ei_object::EiVersion;
use crate::leaks::Tracker;
use crate::wire_ei::EiCallbackId;
use crate::wire_ei::ei_callback::Done;
use crate::wire_ei::ei_callback::EiCallbackRequestHandler;
use std::rc::Rc;
use thiserror::Error;

pub struct EiCallback {
    pub id: EiCallbackId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
}

impl EiCallback {
    pub fn send_done(&self, callback_data: u64) {
        self.client.event(Done {
            self_id: self.id,
            callback_data,
        });
    }
}

impl EiCallbackRequestHandler for EiCallback {
    type Error = EiCallbackError;
}

ei_object_base! {
    self = EiCallback;
    version = self.version;
}

impl EiObject for EiCallback {}

#[derive(Debug, Error)]
pub enum EiCallbackError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
}
efrom!(EiCallbackError, EiClientError);

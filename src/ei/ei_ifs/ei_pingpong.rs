use crate::ei::ei_client::EiClient;
use crate::ei::ei_client::EiClientError;
use crate::ei::ei_object::EiObject;
use crate::ei::ei_object::EiVersion;
use crate::leaks::Tracker;
use crate::wire_ei::EiPingpongId;
use crate::wire_ei::ei_pingpong::Done;
use crate::wire_ei::ei_pingpong::EiPingpongRequestHandler;
use std::rc::Rc;
use thiserror::Error;

#[expect(dead_code)]
pub struct EiPingpong {
    pub id: EiPingpongId,
    pub client: Rc<EiClient>,
    pub tracker: Tracker<Self>,
    pub version: EiVersion,
}

impl EiPingpongRequestHandler for EiPingpong {
    type Error = EiPingpongError;

    fn done(&self, _req: Done, _slf: &Rc<Self>) -> Result<(), Self::Error> {
        Ok(())
    }
}

ei_object_base! {
    self = EiPingpong;
    version = self.version;
}

impl EiObject for EiPingpong {}

#[derive(Debug, Error)]
#[expect(dead_code)]
pub enum EiPingpongError {
    #[error(transparent)]
    EiClientError(Box<EiClientError>),
}
efrom!(EiPingpongError, EiClientError);
